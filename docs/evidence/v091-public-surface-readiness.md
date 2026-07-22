# v0.9.1 public-surface readiness receipt

This packet finishes the v0.9.1 public-surface pass for one exact frozen
runtime without claiming that the candidate has been published or deployed.

## Repository state

- Protected base at source freeze: `a7c00a1a8e48021daf2a9c78cfc1dbda8269e074`
  (`origin/main`, refreshed 2026-07-22)
- Runtime source: `4d197626d72b4bd27e1abf4eed92e86e914414a8`
- Product truth: `508726960`
- Product-first homepage: `e37df06ca`
- Credential-free deploy preflight: `74862148a`
- No Cloudflare deployment, tag, GitHub Release, package publication, or
  artifact publication occurred while producing this packet.

Open PRs were refreshed immediately before sign-off. Draft #4675's remote head
`e2208815e51cdc42830cec8c78d4db1fff00d490`, #4679's head
`7684cbec32a355f72f0a5bd7ef996bdeddf798e0`, and #4680's head
`ccf4c218f619dd9772dd1079f49607faa3504a8e` are unchanged ancestors of the
frozen source, alongside merged PRs #4673 and #4678. No contributor history
was flattened or rewritten. Draft #4508 supplied recovered screenshot intent;
it was not merged wholesale.

The credited fixes for #4681, #4682, the verified strict-DeepSeek boundary in
#4683, and Wenhao Hu's Full Access/global-skill boundary report in #4684 are
also ancestors of the frozen source. The broader intermittent network symptom
reported in #4683 was not reproduced and is not claimed fixed.

## Scope decisions

- Keep the existing Blue Stage visual direction instead of introducing a new
  design system.
- Describe a bounded path from task to verified change, not a perpetual loop.
- Use the existing whale component with a small CSS sun in the community
  section; no generated illustration is shipped.
- Use the real TUI PTY capture from commit
  `4d197626d72b4bd27e1abf4eed92e86e914414a8` as the homepage and README image.
  The two public copies share SHA-256
  `8ffd0c36699930a9af7bcca3e93d3f9bc8a11df5a691e88335fc8b1f0442a754`.

## Visual QA

The production build was inspected in the Codex in-app browser in English and
Chinese at 1280x720 and 390x844, plus the 1012px README content width. Both
mobile pages reported a 390px layout viewport and 390px document width, with
no horizontal overflow; the 1012px review likewise reported a 1012px document
width. The authored locale switch reached `/zh` with `lang=zh`, the mobile menu
opened as a dialog, locked body scrolling, moved focus to its Close control,
and restored the underlying page when closed. The install copy control changed
to `Copied ✓` and rendered its two-pixel focus-visible outline.

The browser context reported `prefers-reduced-motion: false`; it did not expose
a media-emulation capability, so this packet does not mislabel that default
context as a native reduced-motion capture. No running CSS animation was
present in any accepted homepage frame. The source contract separately checks
that the only web animation classes are disabled in the reduced-motion media
query and that the terminal trace stays fully rendered instead of rewinding
when that query matches. The locked real-PTY matrix remains the native proof
for Codewhale's Full, Reduced, and Still motion states. The Open Graph route
returned HTTP 200 as a 1200x630 `image/png` with SHA-256
`a2a03a2fbe32b0e307f159e54d4c94d8b7b83e4cfef1d669b218421e8f8acb11`.

Artifacts:

- `docs/evidence/v091-home-desktop.png`
- `docs/evidence/v091-home-mobile.png`
- `docs/evidence/v091-home-zh-desktop.png`
- `docs/evidence/v091-home-zh-mobile.png`

The four accepted captures have SHA-256 values, in the order above,
`1e6ca263dcae66be0851760ee06675ac825c013c7e93a57f90a7f67abcb63122`,
`af46a2274810d6a6a0bd6c1e2ed74e335678313f1119f513873680764de4cc26`,
`4ae745fa9b93195e7d43a9be36300cb9ec22f5ff6194e05a383732ffce7c8a20`,
and `a00d0e350cf2e2ba7f1eb34f3ba5b94eead20ad09b83b8afddce9e4d8cff5dc5`.

## Verification

```text
npm run lint
npm test -- --run
  17 files passed; 119 tests passed
npm run check:facts
npm run check:docs
npm run check:deploy-env -- --preflight
CODEWHALE_SOURCE_REVISION=4d197626d72b4bd27e1abf4eed92e86e914414a8 \
  npm run compare:deployed-facts
npx tsc --noEmit
CODEWHALE_SOURCE_REVISION=4d197626d72b4bd27e1abf4eed92e86e914414a8 \
  npm run build
CODEWHALE_SOURCE_REVISION=4d197626d72b4bd27e1abf4eed92e86e914414a8 \
  npx opennextjs-cloudflare build
bash scripts/release/check-versions.sh
git diff --check
```

The credential-free deployed-facts report identified the current live gap as
`unavailable` (`/api/facts` returned HTTP 404), while recording the exact
expected source revision, `0.9.1`, 35 providers, 66 tools, and published
release `v0.9.0`; its receipt says `deploymentAttempted: false`. The OpenNext
Cloudflare worker bundle completed successfully. Production deployment was
intentionally not attempted: the local environment does not contain the
protected Cloudflare account ID or API token, and this task does not authorize
a push or deploy.
