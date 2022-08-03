use super::FuncResult;
use crate::server::extract::{AccessBuilder, HandlerContext};
use axum::{extract::Query, Json};
use dal::{Func, FuncBackendKind, FuncId, StandardModel, Visibility};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ListFuncsRequest {
    #[serde(flatten)]
    pub visibility: Visibility,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ListedFuncView {
    pub id: FuncId,
    pub handler: Option<String>,
    pub kind: FuncBackendKind,
    pub name: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ListFuncsResponse {
    pub qualifications: Vec<ListedFuncView>,
}

pub async fn list_funcs(
    HandlerContext(builder, mut txns): HandlerContext,
    AccessBuilder(request_ctx): AccessBuilder,
    Query(request): Query<ListFuncsRequest>,
) -> FuncResult<Json<ListFuncsResponse>> {
    let txns = txns.start().await?;
    let ctx = builder.build(request_ctx.build(request.visibility), &txns);

    let kind = "JsQualification".to_string();
    let qualification_funcs = Func::find_by_attr(&ctx, "backend_kind", &kind)
        .await?
        .iter()
        .map(|func| ListedFuncView {
            id: func.id().to_owned(),
            handler: func.handler().map(|handler| handler.to_owned()),
            kind: func.backend_kind().to_owned(),
            name: func.name().to_owned(),
        })
        .collect();

    txns.commit().await?;

    Ok(Json(ListFuncsResponse {
        qualifications: qualification_funcs,
    }))
}
