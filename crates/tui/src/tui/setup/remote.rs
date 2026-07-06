use crate::localization::Locale;
use crate::tui::app::App;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SetupRemoteFacts {
    pub(super) clouds_result: String,
    pub(super) bridges_result: String,
    pub(super) providers_result: String,
    pub(super) mode_result: String,
    pub(super) result: String,
}

impl SetupRemoteFacts {
    pub(super) fn from_app(app: &App) -> Self {
        let cloud_slugs = crate::remote_setup::registry::CLOUD_TARGETS
            .iter()
            .map(|cloud| cloud.slug)
            .collect::<Vec<_>>();
        let bridge_slugs = crate::remote_setup::registry::BRIDGES
            .iter()
            .map(|bridge| bridge.slug)
            .collect::<Vec<_>>();
        let provider_count = codewhale_config::ProviderKind::all().len();

        Self {
            clouds_result: format!(
                "{} cloud targets: {}",
                cloud_slugs.len(),
                cloud_slugs.join(", ")
            ),
            bridges_result: format!(
                "{} chat bridges: {}",
                bridge_slugs.len(),
                bridge_slugs.join(", ")
            ),
            providers_result: format!(
                "{provider_count} providers from the provider registry; active route {} / {}",
                app.api_provider.as_str(),
                app.model
            ),
            mode_result: format!(
                "generate-only bundle; --apply not implemented; default port {}, workers {}",
                crate::remote_setup::bundle::DEFAULT_PORT,
                crate::remote_setup::bundle::DEFAULT_WORKERS
            ),
            result: format!(
                "clouds={}, bridges={}, providers={}, mode=generate_only, apply=not_implemented",
                cloud_slugs.len(),
                bridge_slugs.len(),
                provider_count
            ),
        }
    }
}

pub(super) fn on_ramp_text(
    locale: Locale,
    clouds_result: &str,
    bridges_result: &str,
    providers_result: &str,
    mode_result: &str,
) -> String {
    let command = "codewhale remote-setup --generate-only --cloud lighthouse --bridge telegram --provider deepseek --out ./codewhale-deploy/lighthouse-telegram";
    match locale {
        Locale::Ja => format!(
            "Remote Runtime On-Ramp\n\n\
             /setup はリモートランタイムの事実だけを表示します。デプロイバンドルの生成、認証情報の書き込み、クラウド CLI の呼び出し、`remote-setup` の実行は行いません。\n\n\
             現在の事実:\n\
             - クラウド: {clouds_result}\n\
             - ブリッジ: {bridges_result}\n\
             - プロバイダー: {providers_result}\n\
             - モード: {mode_result}\n\n\
             デプロイバンドルを生成する場合は、通常の端末で明示的に実行してください:\n\n\
             ```sh\n\
             {command}\n\
             ```\n\n\
             生成された RUNBOOK には人間が確認するホスト手順が含まれます。`--apply` は未実装です。自動デプロイとして扱わないでください。"
        ),
        Locale::ZhHans => format!(
            "Remote Runtime On-Ramp\n\n\
             /setup 只展示远程运行时事实，不会生成部署包、写入凭据、调用云 CLI 或运行 `remote-setup`。\n\n\
             当前事实：\n\
             - 云目标：{clouds_result}\n\
             - 聊天桥：{bridges_result}\n\
             - 服务商：{providers_result}\n\
             - 模式：{mode_result}\n\n\
             生成部署包时，请在普通终端显式运行：\n\n\
             ```sh\n\
             {command}\n\
             ```\n\n\
             生成的 RUNBOOK 会包含需要人工复核的主机步骤。`--apply` 仍未实现；不要把它当成自动部署。"
        ),
        Locale::ZhHant => format!(
            "Remote Runtime On-Ramp\n\n\
             /setup 只顯示遠端執行時事實，不會生成部署包、寫入憑證、呼叫雲端 CLI 或執行 `remote-setup`。\n\n\
             目前事實：\n\
             - 雲端：{clouds_result}\n\
             - 橋接：{bridges_result}\n\
             - 供應商：{providers_result}\n\
             - 模式：{mode_result}\n\n\
             生成部署包時，請在一般終端明確執行：\n\n\
             ```sh\n\
             {command}\n\
             ```\n\n\
             生成的 RUNBOOK 會包含需要人工複核的主機步驟。`--apply` 仍未實作；不要把它當成自動部署。"
        ),
        Locale::PtBr => format!(
            "Remote Runtime On-Ramp\n\n\
             /setup apenas mostra fatos do runtime remoto. Ele não gera bundles, grava credenciais, chama CLIs de cloud nem executa `remote-setup`.\n\n\
             Fatos atuais:\n\
             - Clouds: {clouds_result}\n\
             - Pontes: {bridges_result}\n\
             - Provedores: {providers_result}\n\
             - Modo: {mode_result}\n\n\
             Para gerar um bundle de deploy, execute explicitamente em um terminal normal:\n\n\
             ```sh\n\
             {command}\n\
             ```\n\n\
             O RUNBOOK gerado contém os passos de host para revisão humana. `--apply` continua não implementado; não trate isso como auto-deploy."
        ),
        Locale::Es419 => format!(
            "Remote Runtime On-Ramp\n\n\
             /setup solo muestra datos del runtime remoto. No genera bundles, no escribe credenciales, no llama CLIs de cloud ni ejecuta `remote-setup`.\n\n\
             Datos actuales:\n\
             - Clouds: {clouds_result}\n\
             - Puentes: {bridges_result}\n\
             - Proveedores: {providers_result}\n\
             - Modo: {mode_result}\n\n\
             Para generar un bundle de deploy, ejecútalo explícitamente en una terminal normal:\n\n\
             ```sh\n\
             {command}\n\
             ```\n\n\
             El RUNBOOK generado contiene pasos de host para revisión humana. `--apply` sigue sin implementarse; no lo trates como auto-deploy."
        ),
        Locale::Vi => format!(
            "Remote Runtime On-Ramp\n\n\
             /setup chỉ hiển thị thông tin runtime từ xa. Nó không tạo bundle, ghi thông tin xác thực, gọi CLI cloud hay chạy `remote-setup`.\n\n\
             Thông tin hiện tại:\n\
             - Cloud: {clouds_result}\n\
             - Cầu nối: {bridges_result}\n\
             - Nhà cung cấp: {providers_result}\n\
             - Chế độ: {mode_result}\n\n\
             Để tạo bundle deploy, hãy chạy rõ ràng trong terminal thông thường:\n\n\
             ```sh\n\
             {command}\n\
             ```\n\n\
             RUNBOOK được tạo chứa các bước host để con người xem xét. `--apply` vẫn chưa được triển khai; đừng coi đây là auto-deploy."
        ),
        Locale::En => format!(
            "Remote Runtime On-Ramp\n\n\
             /setup only shows remote-runtime facts. It does not generate bundles, write credentials, call cloud CLIs, or run `remote-setup`.\n\n\
             Current facts:\n\
             - Clouds: {clouds_result}\n\
             - Bridges: {bridges_result}\n\
             - Providers: {providers_result}\n\
             - Mode: {mode_result}\n\n\
             To generate a deploy bundle, run this explicitly in a normal terminal:\n\n\
             ```sh\n\
             {command}\n\
             ```\n\n\
             The generated RUNBOOK contains the host steps for human review. `--apply` remains unimplemented; do not treat it as auto-deploy."
        ),
    }
}
