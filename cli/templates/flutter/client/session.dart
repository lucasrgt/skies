import 'dart:async';

import 'package:skies_flutter/skies_flutter.dart';

SessionSeam createAppSession({
  required void Function(String? token) setAccessToken,
  required Future<AuthTokens?> Function(String refreshToken) refresh,
  required FutureOr<void> Function() clearIdentityCache,
  required FutureOr<void> Function() resetSessionCache,
  required RefreshTokenStore secureStore,
}) => SessionSeam(
  setAccessToken: setAccessToken,
  refresh: refresh,
  onIdentityChanged: clearIdentityCache,
  onSessionChanged: resetSessionCache,
  store: secureStore,
);
