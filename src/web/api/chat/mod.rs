mod guided;
mod notes;
mod stream;
mod suggestions;
mod tabs;

use std::sync::Arc;

use warp::Filter;

use crate::chat_storage::ChatStorage;
use crate::web::api::common::{ApiCtx, ApiRoute};

#[cfg(test)]
pub(crate) use tabs::legacy_chat_types;

pub(crate) fn routes(ctx: ApiCtx, chat_storage: Arc<ChatStorage>) -> ApiRoute {
    let stream = stream::routes(ctx.clone());
    let guided = guided::routes(ctx.clone());
    let suggestions = suggestions::routes(ctx.clone(), chat_storage.clone());
    let notes = notes::routes(ctx.clone());
    let tabs = tabs::routes(ctx, chat_storage);

    stream
        .or(guided)
        .unify()
        .or(suggestions)
        .unify()
        .or(notes)
        .unify()
        .or(tabs)
        .unify()
        .boxed()
}
