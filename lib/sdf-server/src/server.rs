use std::{fmt, future::IntoFuture as _, net::SocketAddr, path::PathBuf, sync::Arc};

use axum::{async_trait, routing::IntoMakeService, Router};
use dal::{JwtPublicSigningKey, ServicesContext};
use hyper::server::accept::Accept;
use nats_multiplexer::Multiplexer;
use nats_multiplexer_client::MultiplexerClient;
use si_data_nats::NatsClient;
use si_posthog::PosthogClient;
use telemetry::prelude::*;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::RwLock,
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use crate::{
    init,
    nats_multiplexer::{CRDT_MULTIPLEXER_SUBJECT, WS_MULTIPLEXER_SUBJECT},
    runnable::Runnable,
    uds::UdsIncomingStream,
    ApplicationRuntimeMode, AxumApp, Config, IncomingStream, Migrator, ServerError, ServerResult,
    WorkspacePermissions, WorkspacePermissionsMode,
};

/// Server metadata, used with telemetry.
#[derive(Clone, Debug)]
pub struct ServerMetadata {
    instance_id: String,
}

impl ServerMetadata {
    /// Returns the server's unique instance id.
    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }
}

pub struct Server {
    metadata: Arc<ServerMetadata>,
    inner: Box<dyn Runnable + Send>,
    // Only used to build a [`Migrator`] for migrations
    services_context: ServicesContext,
    socket: ServerSocket,
}

impl fmt::Debug for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Server")
            .field("metadata", &self.metadata)
            .field("socket", &self.socket)
            .finish()
    }
}

// NOTE(fnichol): This trait trick may only be necessary for the current version of axum--future
// versions may be able to look more like pinga/veritech/rebaser with naxum.
#[async_trait]
impl Runnable for Server {
    async fn try_run(self) -> ServerResult<()> {
        self.inner.try_run().await?;
        info!("sdf main loop shutdown complete");
        Ok(())
    }
}

impl Server {
    #[instrument(name = "sdf.init.from_config", level = "info", skip_all)]
    pub async fn from_config(
        config: Config,
        token: CancellationToken,
        helping_tasks_tracker: &TaskTracker,
        helping_tasks_token: CancellationToken,
    ) -> ServerResult<Self> {
        let (services_context, layer_db_graceful_shutdown) =
            init::services_context_from_config(&config, helping_tasks_token.clone()).await?;

        let jwt_public_signing_key =
            init::load_jwt_public_signing_key(config.jwt_signing_public_key().clone()).await?;
        let (posthog_sender, posthog_client) =
            init::initialize_posthog(config.posthog(), helping_tasks_token.clone())?;

        let (ws_multiplexer, ws_multiplexer_client) = Multiplexer::new(
            services_context.nats_conn(),
            WS_MULTIPLEXER_SUBJECT,
            helping_tasks_token.clone(),
        )
        .await?;
        let (crdt_multiplexer, crdt_multiplexer_client) = Multiplexer::new(
            services_context.nats_conn(),
            CRDT_MULTIPLEXER_SUBJECT,
            helping_tasks_token,
        )
        .await?;

        let application_runtime_mode = Arc::new(RwLock::new(ApplicationRuntimeMode::Running));

        // Spawn helping tasks and track them for graceful shutdown
        helping_tasks_tracker.spawn(layer_db_graceful_shutdown.into_future());
        helping_tasks_tracker.spawn(posthog_sender.run());
        helping_tasks_tracker.spawn(ws_multiplexer.run());
        helping_tasks_tracker.spawn(crdt_multiplexer.run());

        Self::from_services(
            config.instance_id().to_string(),
            config.incoming_stream().clone(),
            services_context,
            jwt_public_signing_key,
            posthog_client,
            config.auth_api_url(),
            ws_multiplexer_client,
            crdt_multiplexer_client,
            *config.create_workspace_permissions(),
            config.create_workspace_allowlist().clone(),
            application_runtime_mode,
            token,
        )
        .await
    }

    #[instrument(name = "sdf.init.from_services", level = "info", skip_all)]
    #[allow(clippy::too_many_arguments)]
    pub async fn from_services(
        instance_id: impl Into<String>,
        incoming_stream: IncomingStream,
        services_context: ServicesContext,
        jwt_public_signing_key: JwtPublicSigningKey,
        posthog_client: PosthogClient,
        auth_api_url: impl AsRef<str>,
        ws_multiplexer_client: MultiplexerClient,
        crdt_multiplexer_client: MultiplexerClient,
        create_workspace_permissions: WorkspacePermissionsMode,
        create_workspace_allowlist: Vec<WorkspacePermissions>,
        application_runtime_mode: Arc<RwLock<ApplicationRuntimeMode>>,
        token: CancellationToken,
    ) -> ServerResult<Self> {
        let app = AxumApp::from_services(
            services_context.clone(),
            jwt_public_signing_key,
            posthog_client,
            auth_api_url,
            ws_multiplexer_client,
            crdt_multiplexer_client,
            create_workspace_permissions,
            create_workspace_allowlist,
            application_runtime_mode,
            token.clone(),
        )
        .into_inner();

        let (inner, socket): (Box<dyn Runnable + Send>, _) = match incoming_stream {
            IncomingStream::TcpSocket(socket_addr) => {
                debug!(%socket_addr, "binding to tcp socket");
                let inner = axum::Server::bind(&socket_addr).serve(app.into_make_service());
                let socket = inner.local_addr();
                info!(%socket, "http service bound to tcp socket");

                (
                    Box::new(InnerServer { inner, token }),
                    ServerSocket::SocketAddr(socket),
                )
            }
            IncomingStream::UnixDomainSocket(path) => {
                debug!(path = %path.display(), "binding to unix domain socket");
                let inner = axum::Server::builder(UdsIncomingStream::create(&path).await?)
                    .serve(app.into_make_service());
                let socket = path;
                info!(socket = %socket.display(), "http service bound to unix domain socket");

                (
                    Box::new(InnerServer { inner, token }),
                    ServerSocket::DomainSocket(socket),
                )
            }
        };

        let metadata = Arc::new(ServerMetadata {
            instance_id: instance_id.into(),
        });

        Ok(Self {
            metadata,
            inner,
            services_context,
            socket,
        })
    }

    #[inline]
    pub async fn run(self) {
        if let Err(err) = self.try_run().await {
            error!(error = ?err, "error while running sdf main loop");
        }
    }

    /// Builds and returns a [`Migrator`] for running migrations.
    pub fn migrator(&self) -> Migrator {
        Migrator::from_services(self.services_context.clone())
    }

    /// Returns the configured [`NatsClient`] used by the server.
    ///
    /// NOTE(fnichol): currently only used to construct a `BillingEventsServer` in the SDF binary.
    /// If this is moved, this method could be removed.
    pub fn nats(&self) -> &NatsClient {
        self.services_context.nats_conn()
    }
}

#[derive(Debug)]
#[remain::sorted]
pub enum ServerSocket {
    DomainSocket(PathBuf),
    SocketAddr(SocketAddr),
}

struct InnerServer<I> {
    inner: axum::Server<I, IntoMakeService<Router>>,
    token: CancellationToken,
}

#[async_trait]
impl<I, IO, IE> Runnable for InnerServer<I>
where
    I: Accept<Conn = IO, Error = IE> + Send + Sync,
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    IE: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    async fn try_run(self) -> ServerResult<()> {
        let token = self.token;

        self.inner
            .with_graceful_shutdown(async {
                token.cancelled().await;
            })
            .await
            .map_err(ServerError::Axum)
    }
}
