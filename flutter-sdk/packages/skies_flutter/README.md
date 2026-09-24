# skies_flutter

The runtime spine for Skies Flutter applications — the Dart mirror of `@skiesjs/react`.

It provides:

- a closed `AsyncState<T>` and `ResourceBuilder<T>` for complete UI-state rendering;
- `executeSkiesRequest` and `SkiesApiException<E>` for typed `dart-dio` failures;
- `SkiesAuthInterceptor` for injected bearer access and one single-flight refresh replay;
- session state, a single-flight session seam, cold-start gating, symmetric route guards, safe back, and required
  route parameters;
- localized stable API errors, visible invalid submit, and a global mutation boundary;
- numbered and accumulated pagination controllers that own state but never requests.

The package does not generate application behavior and does not require a Skies base ViewModel. Use ordinary
`ChangeNotifier` ViewModels and inject generated API operations through constructors.

```dart
final output = await executeSkiesRequest<ListWalletsOutput, ErrorBody>(
  () => api.listWallets(),
  decodeError: (data) => standardSerializers.deserializeWith(
    ErrorBody.serializer,
    data,
  )!,
);
```

Client generation and MVVM scaffolding live in the `skies` CLI. See the
[Flutter conventions](https://github.com/lucasrgt/skies/blob/main/docs/FLUTTER-CONVENTIONS.md).
