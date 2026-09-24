import 'dart:async';
import 'package:flutter_test/flutter_test.dart';
import 'package:skies_flutter/skies_flutter.dart';

void main() {
  for (final signIn in [false, true]) {
    test('old refresh cannot undo ${signIn ? "sign-in" : "logout"}', () async {
      final reply = Completer<AuthTokens?>();
      final started = Completer<void>();
      final store = MemoryStore();
      String? access = 'A';
      var rotations = 0;
      final seam = SessionSeam(
        setAccessToken: (value) => access = value,
        refresh: (_) {
          started.complete();
          return reply.future;
        },
        onIdentityChanged: () {},
        onSessionChanged: () => rotations++,
        store: store,
      );
      final pending = seam.bootstrapSession();
      await started.future;
      if (signIn) {
        await seam.signIn(
          const AuthTokens(accessToken: 'B', refreshToken: 'refresh-B'),
        );
      } else {
        await seam.clearSession();
      }
      reply.complete(
        const AuthTokens(accessToken: 'old-A', refreshToken: 'old-refresh'),
      );
      expect(await pending, isFalse);
      expect(access, signIn ? 'B' : null);
      expect(store.token, signIn ? 'refresh-B' : '');
      expect(rotations, 0);
    });
  }

  test(
    'logout waits for a storage save already in progress and clears it last',
    () async {
      final store = MemoryStore()..saveGate = Completer<void>();
      String? access = 'A';
      final seam = SessionSeam(
        setAccessToken: (value) => access = value,
        refresh: (_) async =>
            const AuthTokens(accessToken: 'old-A', refreshToken: 'old-refresh'),
        onIdentityChanged: () {},
        store: store,
      );
      final pending = seam.bootstrapSession();
      await store.saveStarted.future;
      final logout = seam.clearSession();
      expect(access, isNull);
      store.saveGate!.complete();
      await logout;
      expect(await pending, isFalse);
      expect(access, isNull);
      expect(store.token, isEmpty);
    },
  );

  test('a failed storage write does not poison later sign-out', () async {
    final store = MemoryStore()..failSave = true;
    var cacheClears = 0;
    final seam = SessionSeam(
      setAccessToken: (_) {},
      refresh: (_) async => null,
      onIdentityChanged: () => cacheClears++,
      store: store,
    );
    await expectLater(
      seam.signIn(const AuthTokens(accessToken: 'A', refreshToken: 'r')),
      throwsStateError,
    );
    expect(cacheClears, 1);
    await seam.clearSession();
    expect(store.token, isEmpty);
    expect(cacheClears, 2);
  });
}

final class MemoryStore implements RefreshTokenStore {
  String token = 'refresh-A';
  Completer<void>? saveGate;
  final saveStarted = Completer<void>();
  bool failSave = false;
  @override
  Future<String> load() async => token;
  @override
  Future<void> clear() async => token = '';
  @override
  Future<void> save(String value) async {
    if (!saveStarted.isCompleted) saveStarted.complete();
    if (failSave) throw StateError('storage unavailable');
    final gate = saveGate;
    if (gate != null) await gate.future;
    token = value;
  }
}
