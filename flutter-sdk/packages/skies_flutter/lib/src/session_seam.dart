import 'dart:async';

import 'single_flight.dart';

/// The access and refresh credentials returned by authentication operations.
final class AuthTokens {
  /// Creates an authentication result whose halves may independently be absent.
  const AuthTokens({this.accessToken, this.refreshToken});

  /// The short-lived bearer credential.
  final String? accessToken;

  /// The rotated credential persisted only in secure app-owned storage.
  final String? refreshToken;
}

/// The native secure-storage port used by the session seam.
abstract interface class RefreshTokenStore {
  /// Loads the current refresh credential, or an empty string when absent.
  Future<String> load();

  /// Persists the newest rotated credential.
  Future<void> save(String token);

  /// Clears the credential during sign-out.
  Future<void> clear();
}

/// The only write-side door through which application identity changes.
final class SessionSeam {
  /// Creates the session seam from transport, storage, token, and cache ports.
  SessionSeam({
    required void Function(String? token) setAccessToken,
    required Future<AuthTokens?> Function(String refreshToken) refresh,
    required FutureOr<void> Function() onIdentityChanged,
    FutureOr<void> Function()? onSessionChanged,
    RefreshTokenStore? store,
  }) : _setAccessToken = setAccessToken,
       _refresh = refresh,
       _onIdentityChanged = onIdentityChanged,
       _onSessionChanged = onSessionChanged,
       _store = store ?? const _EmptyRefreshTokenStore() {
    _bootstrap = _refreshForIdentity();
  }

  final void Function(String? token) _setAccessToken;
  final Future<AuthTokens?> Function(String refreshToken) _refresh;
  final FutureOr<void> Function() _onIdentityChanged;
  final FutureOr<void> Function()? _onSessionChanged;
  final RefreshTokenStore _store;
  late SingleFlight<bool> _bootstrap;
  int _identity = 0;
  Future<void> _writes = Future<void>.value();

  /// Persists an explicit sign-in and clears prior-identity caches.
  Future<void> signIn(AuthTokens tokens) {
    final revision = _changeIdentity();
    return _write(() async {
      if (revision != _identity) return;
      try {
        await _store.clear();
        await _persist(tokens, revision);
      } finally {
        if (revision == _identity) await _onIdentityChanged();
      }
    });
  }

  /// Shares one refresh per identity. Replies from an earlier identity are ignored.
  Future<bool> bootstrapSession() => _bootstrap();

  /// Clears credentials after any pending storage write, preventing a late save from restoring them.
  Future<void> clearSession() {
    final revision = _changeIdentity();
    return _write(() async {
      if (revision != _identity) return;
      try {
        await _store.clear();
      } finally {
        if (revision == _identity) await _onIdentityChanged();
      }
    });
  }

  int _changeIdentity() {
    _identity++;
    _setAccessToken(null);
    _bootstrap = _refreshForIdentity();
    return _identity;
  }

  SingleFlight<bool> _refreshForIdentity() {
    final revision = _identity;
    return SingleFlight<bool>(() async {
      try {
        await _writes;
        if (revision != _identity) return false;
        final credential = await _store.load();
        if (revision != _identity) return false;
        final tokens = await _refresh(credential);
        if (tokens?.accessToken == null || tokens!.accessToken!.isEmpty) {
          return false;
        }
        var restored = false;
        await _write(() async {
          if (revision != _identity) return;
          await _persist(tokens, revision);
          if (revision != _identity) return;
          final changed = _onSessionChanged;
          if (changed != null) await changed();
          restored = revision == _identity;
        });
        return restored;
      } on Object {
        return false;
      }
    });
  }

  // Secure storage is asynchronous; serialize writes so sign-out or a newer sign-in always lands last.
  Future<void> _write(Future<void> Function() action) {
    final next = _writes.then((_) => action());
    _writes = next.then<void>((_) {}, onError: (Object _, StackTrace _) {});
    return next;
  }

  Future<void> _persist(AuthTokens tokens, int revision) async {
    final refreshToken = tokens.refreshToken;
    if (refreshToken != null && refreshToken.isNotEmpty) {
      await _store.save(refreshToken);
    }
    if (revision != _identity) return;
    final accessToken = tokens.accessToken;
    _setAccessToken(
      accessToken == null || accessToken.isEmpty ? null : accessToken,
    );
  }
}

final class _EmptyRefreshTokenStore implements RefreshTokenStore {
  const _EmptyRefreshTokenStore();

  @override
  Future<void> clear() async {}

  @override
  Future<String> load() async => '';

  @override
  Future<void> save(String token) async {}
}
