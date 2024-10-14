use axum::{
    extract::{Host, OriginalUri},
    Json,
};
use dal::{AttributeValue, AttributeValueId, ChangeSet, Visibility};
use serde::{Deserialize, Serialize};

use super::ComponentResult;
use crate::{
    extract::{AccessBuilder, HandlerContext, PosthogClient},
    service::force_change_set_response::ForceChangeSetResponse,
    track,
};

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RestoreDefaultFunctionRequest {
    pub attribute_value_id: AttributeValueId,
    #[serde(flatten)]
    pub visibility: Visibility,
}

pub async fn restore_default_function(
    OriginalUri(original_uri): OriginalUri,
    Host(host_name): Host,
    PosthogClient(posthog_client): PosthogClient,
    HandlerContext(builder): HandlerContext,
    AccessBuilder(request_ctx): AccessBuilder,
    Json(request): Json<RestoreDefaultFunctionRequest>,
) -> ComponentResult<ForceChangeSetResponse<()>> {
    let mut ctx = builder.build(request_ctx.build(request.visibility)).await?;

    let force_change_set_id = ChangeSet::force_new(&mut ctx).await?;

    AttributeValue::use_default_prototype(&ctx, request.attribute_value_id).await?;

    track(
        &posthog_client,
        &ctx,
        &original_uri,
        &host_name,
        "restore_default_function",
        serde_json::json!({
            "how": "/component/restore_default_function",
            "attribute_value_id": request.attribute_value_id.clone(),
            "change_set_id": ctx.change_set_id(),
        }),
    );

    ctx.commit().await?;

    Ok(ForceChangeSetResponse::empty(force_change_set_id))
}
