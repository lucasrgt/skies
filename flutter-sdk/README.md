# Skies Flutter SDK

The Flutter side of Skies: `skies_flutter`, a plain Dart runtime that mirrors `@skiesjs/react` (async state,
session, guards, navigation, forms, mutations, API errors, pagination, Dio auth). ViewModels are ordinary
`ChangeNotifier`s; generated `dart-dio` operations are injected through constructors.

Client generation, MVVM scaffolding and the Flutter architecture rules live in the `skies` CLI
(`skies new`, `skies g …`, `skies doctor`), not in this folder.

## Layout

```
flutter-sdk/
  packages/skies_flutter/   # the runtime package (pub)
  openapitools.json         # pinned OpenAPI Generator version for dart-dio client generation
```

## Check

Dart only — no Node toolchain:

```sh
cd packages/skies_flutter
flutter pub get
flutter analyze
flutter test
```

See [Flutter conventions](../docs/FLUTTER-CONVENTIONS.md) for the patterns.
