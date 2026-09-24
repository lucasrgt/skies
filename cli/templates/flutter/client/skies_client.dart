import 'package:dio/dio.dart';
import 'package:skies_flutter/skies_flutter.dart';
import 'package:{{ package }}/{{ package }}.dart';

/// The hand-owned HTTP seam. Generated APIs remain plumbing; auth and error behavior live here.
final class SkiesClient {
  SkiesClient({
    required String baseUrl,
    AccessTokenProvider? accessToken,
    SessionRefresher? refreshSession,
    AuthRoutePredicate? isAuthRoute,
    List<Interceptor> interceptors = const [],
  }) {
    final dio = Dio(BaseOptions(baseUrl: baseUrl));
    final configuredInterceptors = <Interceptor>[...interceptors];
    if (accessToken != null) {
      configuredInterceptors.add(SkiesAuthInterceptor(
        dio: dio,
        accessToken: accessToken,
        refreshSession: refreshSession,
        isAuthRoute: isAuthRoute,
      ));
    }
    api = {{ class }}(dio: dio, interceptors: configuredInterceptors);
  }

  /// The generated API surface. ViewModels reach it only through injected loaders or repositories.
  late final {{ class }} api;

  /// Executes a generated operation and maps its canonical error body.
  Future<T> request<T>(Future<Response<T>> Function() operation) =>
      executeSkiesRequest<T, ErrorBody>(operation, decodeError: _decodeError);

  static ErrorBody _decodeError(Object? data) {
    final error = standardSerializers.deserializeWith(ErrorBody.serializer, data);
    if (error == null) {
      throw const FormatException('The Skies ErrorBody was null.');
    }
    return error;
  }
}
