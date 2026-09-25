# 0008 — Skies 5: opinião sem burocracia

Status: implementado na branch `v5` · 2026-09-23 (ver §10)

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

## 3. Arquitetura do 5.0 (como entregue)

```
skies (binário Rust)                     ← tudo que é tooling
├── new / g …        scaffolders .NET, React, Flutter (templates embutidos, minijinja)
├── doctor           dotnet build (Roslyn) + eslint + regras Flutter nativas + perna workspace, em paralelo
├── spec new         cria .specs/<id>-<slug>/
├── proof run        o green uma vez, julgado por FM; escreve só evidence/raw/ (local)
├── proof record     red (fora da feature) → green (working tree) → receipt.json
└── proof impact     specs que uma mudança alcança: citações do ctx.md + `touches`

Runtime (o que o app importa)
├── .NET     Skies.Framework.{Abstractions, AspNetCore, Auth, EntityFrameworkCore, Identity, Mail, Sms, Storage, Testing}
│            Skies.Framework.Doctor (Roslyn, só arquitetura)
├── React    @skiesjs/react · @skiesjs/eslint-plugin (só arquitetura)
└── Flutter  skies_flutter

Agente
└── skill skies-sdd + AGENTS.md curto
```

### 3.1 O binário Rust

- Um crate `skies-cli` no monorepo (`cli/`), workspace Cargo.
- Distribuição: GitHub Releases (linux/mac/windows) + pacote npm `@skiesjs/cli` com binários por plataforma
  (padrão esbuild) para `npx skies`. A distribuição como `dotnet tool` sai.
- **Scaffolders:** os templates .NET/React/Flutter são minijinja embutidos (`include_dir`); a edição de arquivos
  existentes (registrar módulo, constantes de erro) usa âncoras de texto. Snapshots em `cli/tests/fixtures/`.
- **Doctor:** orquestra processos em paralelo e agrega a saída numa tabela. As regras SKYFL são nativas em Rust
  com `tree-sitter-dart`; a raiz declarada (`[workspace] root`, SKYWS001/002) é lida sem processo.
- **Codegen de client:** chama os geradores de terceiros (openapi-ts / openapi-generator dart-dio).
- **Proof:** relatórios JUnit e TRX com `roxmltree`, git pela CLI (`git worktree`, `git diff`, `git apply`), globs
  com `globset`. Sem hashes, sem coverage, sem paralelismo: roda um spec por vez, quando alguém pede.

### 3.2 Layout de um spec

```
.specs/0012-cancel-reservation/
  spec.md          objetivo, modos de falha (FM-1..n), fora de escopo
  e2e/             testes caixa-preta: HTTP (xUnit), Playwright/Vitest, integration_test
  receipt.json     escrito por `skies proof record`
  red.patch        spec escrito depois do código: o patch que remove a feature
  evidence/        o que os casos salvaram em $SKIES_EVIDENCE (vereditos Assay, screenshot), commitado, ≤ 256 KB
    raw/           relatórios, logs e $SKIES_EVIDENCE/raw/; local, no `.specs/.gitignore`
```

`spec.md`:

```yaml
---
id: "0012"
runner: api            # nome de um runner declarado em Skies.toml
touches: [clients/web/src/reservations/**]   # opcional; só para `proof impact`
---
## Failure modes
- FM-1 cancelar reserva de outro hóspede → 404, reserva intacta
- FM-2 cancelar duas vezes → idempotente, um único estorno [avp: idempotency-key-honored]
- FM-3 cancelar após check-in → 409 `reservation.checked_in`
```

**Uma gramática de FM, em todo lugar** (`report.rs` no motor, `SpecCitations` na SKY0005): no spec, um bullet
`- FM-<n> <texto>` sob `## Failure modes`; num caso, o título começa com `FM-<n>:`/`FM-<n> ` (depois do último
` > `/` › ` de describe) ou o método começa com `FM<n>_`; num ctx.md, a citação `` `0012-cancel-reservation#FM-2` ``.
Parecidos (`FM 3`, `fm_3`, `FM-[x]`) são erro, nunca silêncio. Nada de tags no código de produção, `flows.json` ou
`spec.toml`. O catálogo AVP é o checklist de modos de falha da skill; a tag `[avp: …]` é opcional por FM.

### 3.3 Runners

Declarados em `Skies.toml`. Um runner é um comando de shell (`sh -c`) que roda o `e2e/` de um spec e escreve JUnit
ou TRX em `{report}`:

```toml
[runners.api]
build = "dotnet build tests/App.Tests"
command = "dotnet test tests/App.Tests --no-build --filter FullyQualifiedName~Specs.S{id}. --logger 'trx;LogFileName={report}'"
[runners.web]
setup = "npm --prefix clients/web ci"
command = "node clients/web/node_modules/vitest/vitest.mjs run {dir} --reporter=junit --outputFile={report}"
```

Chaves: `command`, `setup` (uma vez por checkout), `build` (uma vez por checkout, para `--no-build`). Placeholders:
`{id}`, `{spec}`, `{dir}`, `{report}`, `{evidence}` (também `SKIES_EVIDENCE`, e `SKIES_SPEC`). O exit code não
decide nada; o relatório decide. O argumento do logger TRX vai entre aspas: sem elas o `;` termina o comando. Cada
placeholder entra citado conforme onde está (solto, entre `'…'` ou `"…"`), e o comando roda por `sh -c` também no
Windows (o `sh` do Git for Windows, ou `SKIES_SHELL`; sem ele o motor recusa em vez de usar `cmd`).

### 3.4 Recibo

```json
{
  "spec": "0012-cancel-reservation",
  "runner": "api",
  "commands": { "build": "dotnet build …", "command": "dotnet test … --logger '…{report}'", "hash": "blake3:…" },
  "red":   { "commit": "<base>", "patch": "red.patch" },
  "green": { "commit": "<head>" },
  "failure_modes": {
    "FM-1": { "red": "fail", "green": "pass", "cases": ["FM-1: …"], "message": "Expected: NotFound …" },
    "FM-2": { "red": "fail", "green": "pass", "cases": ["FM-2: …"], "avp": ["idempotency-key-honored"],
              "verdict": "evidence/avp-FM-2.json" }
  }
}
```

- **Red:** `--red <rev>`, senão `HEAD` + `red.patch` do spec, senão o merge-base de `HEAD` com `default_branch`,
  `origin/HEAD`, `main` ou `master`, o primeiro que existir. Roda num worktree temporário dentro do repositório
  (`.skies-red/` no topo, no `info/exclude` local, removido ao fim), para que a config acima do projeto
  (NuGet.config, .npmrc, global.json) valha como no green.
  Todo FM tem de falhar; um FM que passa é `non-discriminating` e exige justificativa em `## Non-discriminating`.
- **`did-not-build`, só quando é do spec:** casos que não rodaram contam como falha apenas se a causa está nos
  arquivos do próprio `e2e/` (erro de compilação CS/TS/Dart localizado neles, ou falha de arquivo do relatório,
  como o import não resolvido do vitest); `red.output` cita as linhas. Qualquer outra causa (restore, ferramenta
  ausente, comando quebrado, erro fora do spec, saída vazia) aborta o `record` com exit 2 e sem recibo.
- **Consistência:** todo FM do spec tem caso; todo caso com id está no spec; caso pulado no red ou no green é
  recusado; spec sem FM, ou com o FM placeholder do `spec new`, é recusado; ids de spec duplicados são recusados.
- **Red difere do green só na feature:** o `e2e/` do red é o da working tree, byte a byte; `red.patch` que toca
  `.specs/` ou os arquivos que rodam os testes é recusado, assim como `--red` cujo diff os toca (o red padrão só
  avisa). **Green é um commit:** working tree sujo é recusado, salvo `--allow-dirty`, que grava `"dirty": true`.
- **O que o recibo prova:** os casos nomeados falharam no red e passaram no green sob o runner registrado
  (`commands`); não que as asserções signifiquem algo, o que é papel da revisão e do Assay.
- Sem durações, hashes de relatório nem footprint: regravar um spec sem mudança dá diff zero.

### 3.5 Comandos

| Comando | Faz |
|---|---|
| `skies spec new <slug>` | cria a pasta com `spec.md` modelo (um FM-1 placeholder) e a regra de `evidence/raw/` |
| `skies proof run <id>` | green uma vez, pass/fail por FM com a mensagem dos casos; exit 1 se algum falha |
| `skies proof record <id>` | red → green → `receipt.json` |
| `skies proof impact [paths]` | ctx.md dos módulos tocados → specs citados, mais `touches`; avisa glob que não casa nada |
| `skies doctor` | build + analyzers + lint + regras Flutter + raiz declarada, só arquitetura |

Nenhum deles é plugado em hook pelo template. O CI roda os casos de todo spec a cada push (é ele quem mantém o green).

## 4. Remoções

- **CLI .NET** (`src/Skies.Framework.Cli`, 8.6k LOC, e seus testes, 5k) substituído pelo binário Rust, sem portar
  gate, impacto estático, AVP criteria/scaffolds, foundations/CSM, `FrameworkSync`, `test`/`mutate`.
- **Doctor Roslyn:** saem SKY0003, 0008, 0010, 0011, 0020, 0030–0033, `JourneyProofPolicy`,
  `VerificationDepthPolicy`, `SpecManifest.cs`; SKY0026 vira warning; `Skies.Framework.Testing` perde
  `JourneyAttribute`/`TestCategories`.
- **frontend-sdk:** `tools/` inteiro; regras ESLint de prova (SKYFE005, 006, 008, 033–035) e de design (012,
  024–026); `index.cjs` quebrado em um arquivo por regra.
- **node-sdk:** removido inteiro, com seus docs e o `pilot-api`; os pacotes publicados recebem `npm deprecate`.
- **flutter-sdk:** `tools/` inteiro (as regras viram Rust), `backend_ledger.dart`, paridade e regras de prova.
- **Raiz:** `parity/`, `.aerofortress/`, `taskfleet.toml`, `csm.toml`, `.skies/`, `VERIFICATION.*`, `lefthook.yml`,
  agentes e routines do plugin. `Skies.toml` fica com `[workspace]`, `[products.*]` e `[runners.*]`.

## 5. O que entra

1. **`cli/` (Rust)** conforme §3.
2. **Skill SDD** (`skies-plugin/skills/skies-sdd.md`, copiada no template): entender e rodar `proof impact`;
   escrever `spec.md` e **parar para aprovação humana dos FMs**; escrever os E2E; implementar (`skies doctor` limpo),
   rodando `proof run` assim que os casos compilam; `proof record`; revisar o ctx.md e citar o spec. Todo teste mora
   num spec (SKY0029, SKYFE036, SKYFL036); sistema isolado ganha spec próprio.
3. **Template `skies new`** com `.specs/`, runner `api`, CI que roda os testes, sem hooks.
4. **Sample dogfood:** dez specs com recibos reais (`0001-deposit` … `0010-transfer-screen`), API e web.
5. **`skies migrate 5`**: remove `[AVP]`, `[Journey]`, tags `@verify/@avp/@e2e`, `flows.json`, `*.spec.toml`,
   `VERIFICATION.*`, hooks; declara a raiz; troca a dotnet tool pelo binário.

## 6. Migração dos consumidores (marombas, depois hostpoint)

- Rodar `skies migrate 5`. Os testes existentes não viram specs automaticamente; o SKY0029 aponta onde estão.
- Apagar ou corrigir o que não prova nada (journeys sem assert, `toBeTruthy` redundantes) num PR à parte.
- Feature nova = spec novo com recibo. Opcional: retro-specs para domínios críticos, com `red.patch`.
- Push deixa de subir Docker/emulador.

## 7. Fases

| Fase | Entrega | Estado |
|---|---|---|
| 0 | branch `v5`; 4.x congelado | feito |
| 1 | remoções que não dependem do CLI | feito |
| 2 | `cli/` Rust: `new`, `g *`, `doctor` (+ regras Flutter em tree-sitter) | feito; snapshots substituem a paridade 4.x |
| 3 | `spec new`, `proof run/record/impact` | feito; recibos do sample gravados |
| 4 | skill SDD, AGENTS.md, template, docs, `migrate 5`, release 5.0.0 lockstep | feito, menos o release |
| 5 | marombas e hostpoint migrados | branches `skies-5`, compilando com 0 warnings |

## 8. Riscos

- **O recibo prova red→green uma vez; manter o green é do CI.** Um projeto sem CI rodando os specs perde a
  regressão. Escolha consciente: re-responder isso no motor (staleness, verify, footprint) custou mais do que valeu
  (§10).
- **Red honesto depende do runner.** Um runner que falha antes dos casos por outro motivo agora aborta o `record`
  em vez de gravar `did-not-build`; o custo é configurar `setup` para o checkout de red (ex.: `npm ci`).
- **Menos enforcement:** a qualidade do E2E depende da revisão humana dos FMs. O ponto de controle sai do
  verificador, que o agente aprende a contornar, e vai para o spec, que o humano lê.

## 10. O que mudou durante a implementação

Esta seção descreve o que foi entregue. Histórico: o motor de prova chegou a ter `proof status`, `verify`,
footprint por coverage, impressão por linha, `--with-impacted`, red rot e `--red-only`; tudo isso saiu antes do
release (ver "Motor de prova enxuto").

- **node-sdk removido** e **Rust para todo o tooling** (decisões D1/D2); sem gate, sem hooks, sem noturno (D4).
- **Red que não compila conta como falha, quando é do spec.** E2E que referenciam tipos novos não compilam no
  merge-base; o recibo registra `did-not-build` para todos os FMs e `red.output` cita o compilador, e o log fica em
  `evidence/raw/red.log` (local). Quando todo FM é `did-not-build`, o `record` avisa que o red prova só que os tipos
  são novos.
- **`migrate` preserva `csm.toml` e `.skies/csm`**: são registros do time; só o Skies deixa de rodar as ferramentas.
- **`migrate` copia os helpers de E2E removidos** (fixtures Playwright, ledger de backend, adapter Assay, ledger Dio)
  para dentro da app, para que nenhum teste existente quebre. Provas Assay mantêm suas tags `@avp`.
- **`g crud` gera `Open`/`Update` na entidade** para sair doctor-clean (antes violava SKY0014/SKY0021).
- **Snapshots dos geradores** (`cli/tests/fixtures/snapshots`) substituem a paridade com o 4.x depois do porte.
- **Consumidores**: hostpoint e marombas migrados em branches `skies-5` (worktrees), compilando com 0 warnings.
- **Assay volta acoplado ao spec, opcional por FM.** Uma linha `- FM-n … [avp: <criterion>]` exige, além dos casos,
  o veredito salvo em `$SKIES_EVIDENCE/avp-FM-n.json` com todos os critérios em pass (formato Assay.Net ou TS). Todo
  runner recebe `SKIES_EVIDENCE`/`SKIES_SPEC`; o .NET tem `SpecEvidence.Save` em `Skies.Framework.Testing`.
- **React Native / Expo removido.** React fica só para a web; Flutter é o corpo mobile e também um corpo web
  suportado. O sample junta `core/` + `web/` + `mobile/` num único pacote React web (`frontend/web`); a SKYFE009
  (`viewmodel-platform-agnostic`, que só mantinha ViewModels livres de react-native/expo) sai do plugin e o
  `migrate` remove a configuração dela; as regras de roteamento reconhecem só TanStack Router e React Router; o
  session seam do `@skiesjs/react` perde o `RefreshTokenStore` (na web o refresh é cookie httpOnly).
- **Acessibilidade vira piso ligado por padrão**, no espírito do piso CA* de segurança (só regras estáticas, nunca
  exige teste; §4.5 removeu o A11Y-*obrigatório* de prova, não a leitura estática). Web: o `recommended` do
  `@skiesjs/eslint-plugin` carrega o conjunto recomendado do `eslint-plugin-jsx-a11y` em error (dependência do
  plugin, `aria-role` com `ignoreNonDOM`); a app relaxa uma regra explicitamente num objeto de config posterior, e o
  `migrate` não mexe em ids `jsx-a11y/*`. Flutter: SKYFL037 (`IconButton` sem `tooltip`) e SKYFL038 (imagem sem
  `semanticLabel` nem `excludeFromSemantics`) em error; SKYFL039 (alvo de toque só com ícone, sem rótulo) e SKYFL040
  (campo de texto sem `labelText`/`label`/`hintText`) em warning. Sem gêmeo SKYFE; calibradas nos pacotes Flutter
  do hostpoint sem falso positivo visível.
- **O `.ctx.md` continua obrigatório (SKY0004) e fica vivo por citar os specs.** Uma nota de design cita o spec que
  prova o invariante (`` `0002-withdraw#FM-2` ``); a SKY0005 lê `.specs/*/spec.md` como AdditionalFiles e acusa spec
  ou FM inexistente. `proof impact` lista o ctx de cada módulo tocado e os specs que ele cita.
- **Atritos do dogfood (transferência no sample), corrigidos.** (1) O red padrão escolhia o merge-base com
  `origin/HEAD` = `main`, 116 commits atrás do `v5`, e falhava sem mostrar nada. Agora a ordem é `--red`, `red.patch`
  do spec, `[workspace] default_branch` do Skies.toml, `origin/HEAD`, `main`/`master`; o `record` imprime a escolha e a
  distância (`red 5e2f092 (merge-base with main, default branch from origin/HEAD; 116 commits before HEAD)`) e avisa
  acima de 50 commits; `proof impact` sem caminhos diz o mesmo da sua base. (2) Red que não bate com o spec.md
  imprime o fim da saída do runner e a guarda em `evidence/raw/red.log`. (3) `skies proof run <spec>` roda o green
  uma vez e imprime pass/fail por FM com a mensagem dos casos que falharam. (4) Runner ganhou `build` (uma vez por
  checkout), e o sample compila uma vez e roda `dotnet test --no-build`. (5) `proof impact` imprime o FM inteiro,
  com as linhas de continuação.
- **Raiz declarada (`[workspace] root`, SKYWS001/002).** Decisão do dono: repositório acumula lixo na raiz (logs,
  imagens, docs soltos, pastas avulsas). O `Skies.toml` agora lista tudo o que pode ficar na raiz: globs sobre o nome
  de uma entrada, `/` no fim só casa diretório e sem `/` só casa arquivo; `.git` e `Skies.toml` são implícitos; o que
  o `.gitignore` ignora nunca conta, nem pasta sem nenhum arquivo visível ao git. O `skies doctor` ganhou a perna
  `workspace` (nativa, sem processo, crate `ignore`): SKYWS001 (erro) para entrada não declarada, SKYWS002 (aviso)
  para declaração que não casa nada; manifesto sem `root` recebe um SKYWS001 com a lista atual pronta para colar;
  `--package` não roda. `skies new` escreve a lista do template (o app gerado sai limpo; o `auth-smoke` agora roda o
  próprio `skies doctor`), e `skies migrate 5` declara as entradas atuais e pede para podar. No Hostpoint
  (`hostpoint-skies5`, clone raso): 41 entradas declaradas, entre elas `doctor-rollout.log`, `favicon.png`,
  `.aerofortress/`, `.cursor/`, `.jevd/`, `jevd*.json`, `taskfleet.toml` e `lefthook.yml` para o dono decidir.
- **Evidência compacta, sem churn.** Cada spec commitava os relatórios TRX/JUnit de red e green (~30 KB por spec) e
  todo record os reescrevia. Agora o recibo é o resumo: por FM, em red e green, o resultado, os nomes dos casos e,
  no red, o começo da mensagem do primeiro caso que falhou (a asserção, sem stack); `red.output` diz por que o red
  não compilou. Relatórios e logs vão para `evidence/raw/`, que o motor mantém fora do git (acrescenta
  `/*/evidence/raw/` ao `.specs/.gitignore`); commitado fica só o que o caso salvou em `$SKIES_EVIDENCE` (vereditos
  Assay, screenshot, log HTTP), até 256 KB por arquivo. Sem duração nem timestamp no recibo; veredito que só muda em
  `durationMs` mantém os bytes, e regravar um spec sem mudança deixa o git limpo.
- **A mecânica de auth vira pacote.** O `g auth` (e `auth:otp`/`auth:oauth`/`auth:email`) copiava para cada app o
  hash de senha, a rotação do refresh com queima da família no reuso, a revogação, a entrega por cookie, o timing do
  login, a emissão/verificação de tokens de email e códigos de SMS; uma correção no template nunca chegava a um app
  já gerado. Agora é o `Skies.Framework.Auth`: `IPasswordHasher` (argon2id, mesmo formato do Skies 4),
  `OpaqueTokens` (comparação em tempo constante), `RefreshSessions`, `VerificationTokens`, `RefreshCookie.Deliver`,
  registrados por uma chamada explícita (`AddSkiesAuth<UserSessionStore>` no `AccountSetup`, mais
  `AddVerificationTokens<VerificationTokenStore>` com um fluxo de telefone/email); o `Identity` ganhou a porta
  assíncrona `IExternalIdentityVerifier` e o `OidcIdTokenVerifier` (JWKS, issuer, audience, validade, email
  verificado; só algoritmos assimétricos). O app mantém entidades e tabelas (`User`, `UserSession`, um
  `VerificationToken` no lugar de `PhoneOtp`/`EmailVerificationToken`/`PasswordResetToken`) atrás de dois stores
  pequenos (`IRefreshSessionStore`, `IVerificationStore`), sem classe base, sem DbContext do framework, sem geração;
  as slices mantêm Input/Output/Handle/Map, os códigos de erro (mapeados dos enums de resultado) e a postura de auth.
  Os 38 casos dos quatro specs passam com edições mecânicas só em três arquivos do `0001-auth` (assinaturas de
  `Handle` e `SessionToken.*` → `RefreshSessionOptions.Default.*`). Código gerado: Full 1610 → 1373 linhas de C# na
  API (slices 753 → 652), Single 781 → 678; nenhuma decisão de segurança fica no app (0 linhas de cripto, rotação ou
  contagem de tentativas). O `migrate 5` não reescreve auth: aponta um `Refresh.cs` do Skies 4 com uma nota.
- **Motor de prova enxuto (decisão do dono após auditoria adversarial).** O motor tinha passado do gate do Skies 4
  que substituiu (~5,3k linhas não-teste em 29 arquivos, ~35 conceitos). O CI gerado já roda `dotnet test`/`npm test`
  e executa o green de todo spec a cada push, então staleness, `verify`, footprint por coverage, impressão por linha,
  `--with-impacted`, `verified_with`, red rot e `--red-only` respondiam de novo o que o CI responde; só o red é
  evidência única. Ficam `spec new`, `proof run` (green uma vez, nada commitado), `proof record` (red falha todo FM,
  `did-not-build` para qualquer runner; green passa todo FM; `## Non-discriminating`; checagem de consistência; Assay
  `[avp: …]`) e `proof impact` derivado das citações do ctx.md (`**/Modules/<M>/` → `<M>.ctx.md` → specs citados) mais
  `touches:`. O recibo é por FM: red e green, os casos, a primeira mensagem do red, critérios e veredito Assay; commits
  de red (+ `red.patch`) e green; o runner. Sem durações, hashes de relatório, footprint nem `ctx_revised`; regravar
  um spec sem mudança dá diff zero (verificado nos 10 do sample). Runner: `command`, `setup`, `build`; saem `report`, `env`,
  `coverage`, `scope` e `SKIES_COVERAGE`, coverlet e `@vitest/coverage-v8`. `status` e `verify` saem da CLI. Motor:
  5 273 → ~2 500 linhas não-teste, 29 → 16 arquivos.
- **Recibo que prova algo (auditoria independente).** Um recibo podia não provar nada: (1) qualquer falha de red
  sem relatório virava `did-not-build` em todo FM, inclusive restore quebrado (o red rodava em `/tmp/skies-red-*`,
  fora do alcance do NuGet.config acima do repositório), ferramenta ausente ou comando de runner inválido. Agora
  `did-not-build` exige causa nos arquivos do próprio `e2e/` (erro CS/TS/Dart localizado neles, ou falha de arquivo
  do relatório cuja mensagem não aponta um caminho de fora, como o `from` do import do vitest); o resto aborta com
  exit 2, a saída e sem recibo. (2) O red roda em `.skies-red/<id>` no topo do repositório (no `info/exclude` local,
  sempre removido). A primeira tentativa, dentro de `.git/`, quebrou o vitest (o Vite nega `**/.git/**`) e o
  classificador ainda aceitava o `Cannot find module '/frontend-sdk/vitest.setup.ts'` como culpa do spec; os dois
  foram corrigidos e o `record` do 0010 voltou a dar `fail` real em todo FM. (3) Spec sem FM, ou ainda com o
  FM-1 placeholder do `spec new`, é recusado por `run` e `record`. (4) Uma gramática de FM (§3.2)
  igual à da SKY0005; parecidos são erro. O `auth-smoke` escrevia `FM-[rejected-update]` (zero FMs para o motor) e só
  contava DisplayNames; agora usa FMs numéricos e roda `skies proof run` em cada spec conferindo `N/N FMs pass`
  contra as linhas do spec.md. (5) `--logger trx;LogFileName={report}` sem aspas terminava o `sh -c` no `;` em docs e
  mensagens; tudo entre aspas, com teste que roda o runner do template e do sample num `dotnet` falso. (6) `touches`
  que não casa nada vira aviso em `impact`/`record`; `$SKIES_EVIDENCE/raw/` é `evidence/raw/` também no `run`, que
  não escreve mais nada fora dali (o `.specs/.gitignore` passou para `spec new`/`record`); a cadeia do red perdeu o
  upstream do branch (`--red`, `red.patch`, `default_branch`, `origin/HEAD`, `main`/`master`).
- **Recibo que não se forja (segunda auditoria independente).** (1) Caso pulado no red contava como falha, então um
  `it.skipIf`/`test.skip(cond)` que só vale no red forjava o red; agora caso pulado no red ou no green aborta o
  `record`. (2) O `red.patch` era aplicado depois de copiar o `e2e/` e podia reescrever os casos do próprio spec;
  agora um patch que toca `.specs/` ou os arquivos que rodam os testes (o projeto `tests`, arquivo citado no comando
  do runner e os setup files da config dele, `vitest.config.*`, `*.Tests.csproj`…) é recusado, e um `--red` cujo
  diff para o green toca esses arquivos ou os compartilhados de `.specs/` também; o red padrão só avisa. (3) O
  recibo passa a fixar o que o produziu: `commands` (o `setup`/`build`/`command` do runner como escritos, mais um
  hash blake3) e um green que é commit: `record` recusa working tree sujo, salvo `--allow-dirty`, que grava
  `"dirty": true`. Um caso que distingue red de green por onde roda (`$PWD`) continua fora do alcance do motor; a
  revisão é quem pega. (4) CONVENTIONS e a skill dizem o que o recibo prova (os casos falharam no red e passaram no
  green sob o runner registrado) e o que não prova (que as asserções significam algo: revisão e Assay). (5) Ids de
  spec duplicados são recusados em `run`/`record`/`impact` (o `web-smoke` criava `0001-home` ao lado de `0001-auth`
  e rodava o runner da API; agora usa `spec new` e confere na saída que o runner web rodou). (6) Runner roda por
  `sh -c` em toda plataforma (Git Bash ou `SKIES_SHELL` no Windows, nunca `cmd /C`) e cada placeholder entra citado
  conforme o contexto, então caminho com espaço funciona. (7) O FM placeholder do `spec new` é recusado. (8) A
  gramática lê só o título do caso, então um `describe` chamado `FM-2: …` não transforma os casos em parecidos.
