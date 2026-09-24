# 0008 — Skies 5: opinião sem burocracia

Status: proposta · 2026-09-23

## 1. Por que (evidência, não opinião)

| Sintoma | Medida |
|---|---|
| O harness engoliu o framework | CLI .NET 8.6k LOC, dos quais ~4.2k são gate/manifest; bibliotecas de runtime .NET ~2.3k no total |
| Churn no lugar errado | 60 dias: ~10.8k linhas em CLI/gate/analyzers/parity vs ~1.4k em runtime .NET e ~0.5k em `frontend-sdk/packages` |
| Gate lento e frágil | hostpoint: pre-push sobe Docker/emulador + `check --base` sem `--fast`; último `attempt.json` = `failed-exit-127` com `active.lock` órfão |
| Prova que não prova | 8 happy journeys sem assert; `LookupHost` "role-required" nunca testa o caminho autorizado; 87 `getBy…toBeTruthy()`; `flows.json` usa copy de UI como "evidence" (commits "align proofs with current copy") |
| Spec desconectada de teste | ids do spec (`A1-total-soma`…) aparecem em 0 arquivos; os 225 `[AVP]` inventam seus próprios ids |
| Um critério, cinco lugares | `@verify` → `@avp` → `defineVerification` → `flows.json.criteria` → `spec.toml`/`skies.node.json` |
| Tudo reimplementado 3-4x | impact selection (C#, TS import graph, Node glob engine), journey rules (.NET, Node, React, Flutter), framework-sync (3x), glob→regex (3x) |
| Versões espalhadas | hostpoint: CLI 4.1.15, .NET 4.1.4, frontend-sdk 4.1.17, react 4.0.5; arquivos de versão são os mais churnados do repo |
| Resto morto | 22/24 deferments de parity expirados; `requireSeed` inexistente; `aggregateReport` sem chamador; `.aerofortress`, telemetria `tool-compiler`, README "Pleiades"; sample viola as próprias regras de E2E |

Diagnóstico: o gate tenta responder "o que rodar para este diff" com análise estática (um Bazel caseiro) e tenta
impedir o agente de trapacear com contratos cada vez mais finos. As duas frentes produzem complexidade, e a
segunda produz slop: o agente otimiza para o verificador.

## 2. Princípios do 5.0

1. **Leis mantidas:** stranger-maintainable e doctor-removable.
2. **Opinativo, não burocrático.** O framework diz *como* construir (shape de slice, MVVM, módulos, erros, auth) e
   o compilador/lint acusa desvio. Ele não exige cerimônia, não bloqueia push, não audita o agente.
3. **Sem gate.** Nada roda em hook por obrigação. Os analyzers já rodam no `dotnet build`, o ESLint no editor e no
   `lint`. Não existe `check`, retry-review, varredura de supressões, sync de versões, policiamento de CI.
4. **Evidência no spec.** Uma funcionalidade é entregue com um recibo reproduzível na pasta do spec, não com
   anotações espalhadas pelo código de produção. O recibo é registro, não catraca.
5. **Modos de falha antes do código.** O spec enumera como a feature pode falhar; cada modo vira um caso E2E que
   falha antes da implementação (red) e passa depois (green).
6. **Rust onde der, C# onde precisa.** Todo o tooling é um binário Rust. C# fica só no que roda dentro do .NET
   (bibliotecas de runtime e analyzers Roslyn); TS só no runtime React e no plugin ESLint; Dart só no runtime Flutter.
7. **Um release, uma versão.** Todos os pacotes (NuGet, npm, pub, binário) saem em lockstep como `5.x.y`.

## 3. Arquitetura do 5.0

```
skies (binário Rust)                     ← tudo que é tooling
├── new / g …        scaffolders .NET, React, Flutter (templates atuais reaproveitados via minijinja)
├── doctor           orquestra dotnet build (Roslyn) + eslint + regras Flutter nativas, em paralelo
├── spec new         cria .specs/<id>/
└── proof record | verify | status

Runtime (o que o app importa)
├── .NET     Skies.Framework.{Abstractions, AspNetCore, Auth, EntityFrameworkCore, Identity, Mail, Sms, Storage}
│            Skies.Framework.Testing.Postgres
│            Skies.Framework.Doctor (Roslyn, só arquitetura)
├── React    @skiesjs/react · @skiesjs/eslint-plugin (só arquitetura)
└── Flutter  skies_flutter

Agente
└── skill skies-sdd + AGENTS.md curto
```

### 3.1 O binário Rust

- Um crate `skies-cli` no monorepo (`cli/`), workspace Cargo.
- Distribuição: GitHub Releases via `cargo-dist` (linux/mac/windows) + pacote npm `@skiesjs/cli` com binários
  por plataforma (padrão esbuild) para `npx skies`. A distribuição como `dotnet tool` sai.
- **Scaffolders:** os 85 `*.cstmpl` e os templates de React/Flutter viram templates minijinja embutidos
  (`include_str!`). A lógica de edição (registrar módulo, constantes de erro) usa âncoras de texto como hoje.
- **Doctor:** orquestra processos em paralelo e agrega a saída numa tabela só. As regras SKYFL (hoje regex em
  `flutter-sdk/tools/doctor.mjs`) passam a ser nativas em Rust com `tree-sitter-dart`: parser de verdade em
  vez de regex, milissegundos por arquivo. Checagens rápidas que valem a pena manter (i18n com as mesmas chaves em
  todos os locales, ctx.md citando código que existe no front) também viram Rust.
- **Codegen de client:** continua usando os geradores de terceiros (openapi-ts / openapi-generator dart-dio); o
  Rust só chama e confere que a saída mudou.
- **Proof:** hashing com `blake3`, JUnit com `quick-xml`, git via CLI (`git worktree`, `git diff`), paralelismo com `rayon`.

### 3.2 Layout de um spec

```
.specs/0012-cancel-reservation/
  spec.md          objetivo, comportamento, modos de falha (FM-1..n), fora de escopo
  e2e/             testes caixa-preta: Playwright, HTTP (xUnit), integration_test, Maestro
  receipt.json     gerado por `skies proof record`
  evidence/        junit.xml, log HTTP, screenshots dos estados terminais (pequeno)
```

Traces e vídeos ficam fora do git (`.gitignore`), regeneráveis por `proof record`.

`spec.md`:

```yaml
---
id: 0012
runner: web            # nome de um runner declarado em Skies.toml
touches: [src/Hostpoint.Api/Modules/Reservations/**]   # opcional; amplia o footprint
---
## Failure modes
- FM-1 cancelar reserva de outro hóspede → 404, reserva intacta
- FM-2 cancelar duas vezes → idempotente, um único estorno
- FM-3 cancelar após check-in → 409 `reservation.checked_in`
```

**Único mecanismo de ligação:** o título do caso de teste começa com o id (`test("FM-2: double cancel refunds once")`).
Nada de tags no código de produção, nada de `flows.json`, nada de `spec.toml`.

O catálogo do AVP deixa de ser dependência e vira o **checklist de modos de falha** da skill SDD (idempotência,
autorização no servidor, estado intacto no erro, concorrência, dinheiro, navegação…).

### 3.3 Runners

Declarados em `Skies.toml`. Um runner é um comando que recebe a pasta `e2e/` e emite JUnit:

```toml
[runners.api]
command = "dotnet test tests/Specs.Tests --logger junit;LogFilePath={junit} -- Specs.Dir={dir}"
[runners.web]
command = "npx playwright test {dir} --reporter=junit"
env = { PLAYWRIGHT_JUNIT_OUTPUT_NAME = "{junit}" }
setup = "npm run stack:up"      # opcional
```

O motor não conhece xUnit, Playwright nem Flutter: lê JUnit. Isso apaga `FrontendGate`, `FrontendScriptContract`,
`playwright-affected`, `assay-affected`, `node-test-affected` e o suporte específico a Maestro.

### 3.4 Recibo

```json
{
  "spec": "0012-cancel-reservation",
  "runner": "web",
  "footprint": { "src/.../CancelReservation.cs": "blake3:…" },
  "inputs":    { "spec.md": "blake3:…", "e2e/": "blake3:…", "package-lock.json": "blake3:…" },
  "red":   { "commit": "<base>", "cases": { "FM-1": "fail", "FM-2": "fail", "FM-3": "fail" } },
  "green": { "commit": "<head>", "cases": { "FM-1": "pass", "FM-2": "pass", "FM-3": "pass" }, "junit": "evidence/junit.xml" }
}
```

- **Footprint:** arquivos alterados entre `red.commit` e `green.commit` + `touches` + a pasta do spec + lockfiles.
  (v2: arquivos realmente executados, via coverage.)
- **Red:** todo FM tem de falhar no commit base. Um FM que passa no base é gravado como `non-discriminating` e o
  `record` pede uma linha de justificativa no spec. É o antídoto contra teste que não morde.
- **Consistência:** todo FM do spec tem um caso; todo caso `FM-*` está no spec. É a única checagem do sistema.

### 3.5 Comandos

| Comando | Faz |
|---|---|
| `skies spec new <slug>` | cria a pasta com `spec.md` modelo |
| `skies proof record <id>` | worktree no base → roda (red) → roda no head (green) → escreve recibo |
| `skies proof status` | só hashes, milissegundos: lista recibos válidos e stale. Não executa nada, não falha |
| `skies proof verify [<id>…\|--stale\|--all]` | reroda os specs pedidos e atualiza `green` |
| `skies doctor` | build + analyzers + lint + regras Flutter, só arquitetura |

Nenhum deles é plugado em hook pelo template. Quem quiser, pluga.

## 4. Remoções

### 4.1 CLI .NET — sai inteiro
`src/Skies.Framework.Cli` (8.6k LOC) e `tests/Skies.Framework.Cli.Tests` (5k LOC) são substituídos pelo binário Rust.
O que **não** é portado:
- Gate: `GateCommand`, `GateOptions`, `GateScan`, `GateImpact*`, `CSharpImpactGraph`, `GeneratedClientImpact`,
  `FrontendConsumers`, `GateMatrix`, `GateReport`, `GateAttempt`, `SuppressionGate`, `FrontendGate`,
  `FrontendScriptContract`, `FrontendWarningGate`, `FlutterIntegrationSuite`, `GitChanges`, `Tools/*.mjs`, `VERIFICATION.*`.
- AVP: `criteria`, `CriteriaHeuristic`, `AvpBinding`, `AvpProofScaffold`, `SpecManifestScaffold`, `CrudAcceptance`,
  `--verify` do `g slice`, `*.spec.toml`.
- Foundations/CSM: `foundations`, `context`, `check`, `nya|wtw|rtw|nwc`, download de binário, `csm.toml`, reescrita de
  AGENTS.md. As ferramentas why-this-way/right-this-way/not-you-again/now-we-can seguem como produtos à parte.
- `FrameworkSync`, `FrameworkPackageVersions`, `SkiesManifest.CheckGateWorkflow`, validação de `launchSettings`,
  `DesignHarness`, `test`/`mutate`.

### 4.2 Doctor Roslyn (~1.3k LOC de cerimônia)
Sai: SKY0003, 0008, 0010, 0011, 0020, 0030, 0031, 0032, 0033, `JourneyProofPolicy`, `VerificationDepthPolicy`,
`SpecManifest.cs`.
Fica: SKY0001, 0002, 0004, 0005, 0006, 0007, 0009, 0012–0019, 0021–0028, globalconfig CA*.
SKY0026 (token de concorrência) cai para warning: 58 ocorrências no rollout do hostpoint, virou ruído.
`Skies.Framework.Testing`: saem `JourneyAttribute` e `TestCategories`; fica `Testing.Postgres`.

### 4.3 frontend-sdk
- `tools/` sai inteiro. Scaffold/generate/i18n migram para o Rust; o resto era gate: `e2e-doctor`,
  `feature-e2e-coverage`, `playwright-backend`, `playwright.mjs`, `product-verification`, `eslint-gate`,
  `journey-parity`, `endpoint-coverage`, `error-code-coverage`, `contract-freshness`, `doctor.mjs`,
  `framework-sync`, `package-versions`, `release-guard`, `object-literals`, `tests/cli-*`.
- ESLint: saem SKYFE005, 006, 008, 033, 034, 035 (prova) e 012, 024, 025, 026 (design). `index.cjs` (1.9k LOC num
  arquivo) é quebrado em um arquivo por regra.
- Design: `design-scaffold`, `ui-kit-web`, canonical screens, `design:rendered`, `docs/DESIGN-CONVENTIONS.md`,
  `.design/`, skill `skies-design`.

### 4.4 node-sdk — removido
Pasta inteira, `docs/NODE-CONVENTIONS.md`, `docs/NODE-PARITY.md`, `node-sdk/AGENTS.md`, exemplo `pilot-api`.
Os pacotes `@skiesjs/{core,express,…}` publicados recebem `npm deprecate` apontando para o 4.x final.

### 4.5 flutter-sdk
- `tools/` sai inteiro: regras viram Rust (§3.1), scaffolds viram templates Rust, o resto era gate
  (`e2e-doctor`, `feature-e2e-coverage`, `journey-parity`, `endpoint-coverage`, `framework-sync`, `contract-smoke`,
  `parity.mjs`, `design-scaffold`).
- `backend_ledger.dart`, `flutter-react.parity.json`, regras SKYFL de prova, design e A11Y-obrigatório.

### 4.6 Raiz
- `parity/`, `tools/parity-guard.mjs`, job `parity` do CI.
- `.aerofortress/`, `taskfleet.toml`, `csm.toml`, `.skies/` inteiro, `VERIFICATION.*`, `docs/PORTBACK-CHECKLIST.md`,
  `docs/audits/`, `lefthook.yml` (sem hooks obrigatórios).
- `skies-plugin/agents/*` (4 agentes) e `routines/` → absorvidos pela skill SDD.
- `AGENTS.md`: bloco `skies:foundations` sai; manual cai para ~80 linhas.
- `Skies.toml`: só `[workspace]`, `[products.*]` e `[runners.*]`.

Estimativa grosseira: −25k LOC de código e −12k de testes (incluindo node-sdk). Entra ~6k de Rust
(scaffolders ~2.5k, doctor + regras Flutter ~2k, proof ~1.5k).

## 5. O que entra

1. **`cli/` (Rust)** conforme §3.1 e §3.5.
2. **Skill SDD** (`skies-plugin/skills/skies-sdd.md`), substitui `skies-feature`:
   1. Entender o pedido; ler `.ctx.md` do módulo.
   2. Escrever `spec.md`: comportamento + modos de falha (checklist AVP). **Parar para aprovação humana dos FMs.**
   3. Escrever os E2E em `e2e/`, um caso por FM, título `FM-n: …`. Rodar: todos devem falhar.
   4. Scaffold + implementação (`skies doctor` limpo).
   5. `skies proof record`. Entregar o recibo como relatório.
   Regras: sem teste unitário escrito depois do código; unitário só para sistema isolado, com modos de falha escritos
   antes, e aí ele é um teste comum, fora dos recibos.
3. **Template `skies new`** com `.specs/`, runners de exemplo, sem hooks, sem `csm.toml`/`.design`/`VERIFICATION`.
4. **Sample dogfood:** `Deposit` e `Withdraw` viram `.specs/0001-deposit` e `0002-withdraw` com recibos reais,
   substituindo os `*.Avp.Tests.cs`.
5. **`skies migrate 5`** (Rust): remove `[AVP]`, `[Journey]` (vira `[Fact]`), tags `@verify/@avp/@e2e`, `flows.json`,
   `*.spec.toml`, `VERIFICATION.*`, `csm.toml`, `.skies/csm`, bloco de AGENTS.md, hooks do lefthook; troca a
   dotnet tool pelo binário.

## 6. Migração dos consumidores (marombas, depois hostpoint)

- Rodar `skies migrate 5`. Os testes existentes **não viram specs**: continuam como testes comuns, rodados quando
  alguém quiser (`dotnet test`, `npm test`, `flutter test`).
- Apagar ou corrigir o que não prova nada (8 journeys sem assert, `toBeTruthy` redundantes) num PR à parte.
- Feature nova = spec novo com recibo. Opcional: retro-specs para domínios críticos (escrow, reservas), com red
  contra um commit que reintroduz o bug.
- Push deixa de subir Docker/emulador.

## 7. Fases

| Fase | Entrega | Pronto quando |
|---|---|---|
| 0 | branch `v5`; 4.x congelado | — |
| 1 | remoções que não dependem do CLI: node-sdk, parity, design, regras de prova (Roslyn/ESLint), `frontend-sdk/tools` de gate, sobras da raiz | `dotnet build/test` e testes dos SDKs verdes |
| 2 | `cli/` Rust: `new`, `g *`, `doctor` (+ regras Flutter em tree-sitter) | `skies new` + `g slice/crud/auth` geram saída byte-idêntica ao CLI 4.x (menos arquivos de prova); CLI .NET apagado |
| 3 | `spec new`, `proof record/status/verify` | recibos do sample gerados; `status` < 100 ms no hostpoint |
| 4 | skill SDD, AGENTS.md, template, docs, `migrate 5`, release 5.0.0 lockstep | feature feita do zero via skill num app limpo |
| 5 | marombas migrado, depois hostpoint | push sem Docker; primeira feature nova com recibo |

A fase 2 é a maior. A paridade byte-a-byte com os templates 4.x é o teste: gerar com os dois CLIs e comparar a
árvore (um spec do próprio framework, com recibo).

## 8. Riscos

- **Sem rerun automático, regressão só aparece quando alguém roda.** Escolha consciente. `proof status` torna os
  recibos stale visíveis em milissegundos; `proof verify --stale` é o comando para quando importar.
- **Footprint v1 é estreito:** mudança num middleware compartilhado não marca como stale os recibos que o usam.
  Mitigação futura: footprint por coverage.
- **Reescrita do CLI em Rust** é o item de maior custo. Mitigação: templates reaproveitados, paridade byte-a-byte
  como critério, e o CLI 4.x continua funcionando até a fase 2 fechar.
- **Menos enforcement:** a qualidade do E2E depende da revisão humana dos FMs. O ponto de controle sai do
  verificador, que o agente aprende a contornar, e vai para o spec, que o humano lê.
