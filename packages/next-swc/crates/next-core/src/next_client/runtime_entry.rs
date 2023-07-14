use anyhow::{bail, Result};
use turbopack_binding::{
    turbo::{tasks::ValueToString, tasks_fs::FileSystemPathVc},
    turbopack::{
        core::{
            chunk::{EvaluatableAssetVc, EvaluatableAssetsVc},
            context::AssetContextVc,
            issue::{IssueSeverity, OptionIssueSourceVc},
            module::{convert_asset_to_module, Module},
            resolve::{origin::PlainResolveOriginVc, parse::RequestVc},
            source::SourceVc,
        },
        ecmascript::resolve::cjs_resolve,
    },
};

#[turbo_tasks::value(shared)]
pub enum RuntimeEntry {
    Request(RequestVc, FileSystemPathVc),
    Evaluatable(EvaluatableAssetVc),
    Source(SourceVc),
}

#[turbo_tasks::value_impl]
impl RuntimeEntryVc {
    #[turbo_tasks::function]
    pub async fn resolve_entry(self, context: AssetContextVc) -> Result<EvaluatableAssetsVc> {
        let (request, path) = match *self.await? {
            RuntimeEntry::Evaluatable(e) => return Ok(EvaluatableAssetsVc::one(e)),
            RuntimeEntry::Source(source) => {
                return Ok(EvaluatableAssetsVc::one(EvaluatableAssetVc::from_source(
                    source, context,
                )));
            }
            RuntimeEntry::Request(r, path) => (r, path),
        };

        let assets = cjs_resolve(
            PlainResolveOriginVc::new(context, path).into(),
            request,
            OptionIssueSourceVc::none(),
            IssueSeverity::Error.cell(),
        )
        .primary_assets()
        .await?;

        let mut runtime_entries = Vec::with_capacity(assets.len());
        for &asset in &assets {
            if let Some(entry) = EvaluatableAssetVc::resolve_from(asset).await? {
                runtime_entries.push(entry);
            } else {
                let asset = convert_asset_to_module(asset);
                bail!(
                    "runtime reference resolved to an asset ({}) that cannot be evaluated",
                    asset.ident().to_string().await?
                );
            }
        }

        Ok(EvaluatableAssetsVc::cell(runtime_entries))
    }
}

#[turbo_tasks::value(transparent)]
pub struct RuntimeEntries(Vec<RuntimeEntryVc>);

#[turbo_tasks::value_impl]
impl RuntimeEntriesVc {
    #[turbo_tasks::function]
    pub async fn resolve_entries(self, context: AssetContextVc) -> Result<EvaluatableAssetsVc> {
        let mut runtime_entries = Vec::new();

        for reference in &self.await? {
            let resolved_entries = reference.resolve_entry(context).await?;
            runtime_entries.extend(&resolved_entries);
        }

        Ok(EvaluatableAssetsVc::cell(runtime_entries))
    }
}
