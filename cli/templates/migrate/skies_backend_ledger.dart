import 'package:dio/dio.dart';

/// The expected transport outcome of one backend operation.
enum BackendOutcome {
  /// A response whose status is between 200 and 399.
  success,

  /// A response whose status is 400 or greater.
  error,
}

/// One OpenAPI operation mapped to its HTTP method and path template.
final class BackendOperation {
  /// Creates an operation description derived from the checked-in contract.
  BackendOperation({
    required this.operationId,
    required String method,
    required String pathTemplate,
  }) : method = method.toUpperCase(),
       _path = _compilePath(pathTemplate);

  /// The stable OpenAPI operationId.
  final String operationId;

  /// The uppercase HTTP method.
  final String method;

  final RegExp _path;

  bool _matches(RequestOptions request) =>
      request.method.toUpperCase() == method &&
      _path.hasMatch(request.uri.path);
}

/// One real response observed through the application's Dio instance.
final class BackendObservation {
  /// Creates an immutable backend observation.
  const BackendObservation({
    required this.operationId,
    required this.statusCode,
  });

  /// The matched OpenAPI operationId.
  final String operationId;

  /// The real HTTP response status.
  final int statusCode;

  /// Whether the response is a successful application outcome.
  bool get isSuccess => statusCode >= 200 && statusCode < 400;
}

/// Records real backend operations during a Flutter integration test.
final class BackendLedgerInterceptor extends Interceptor {
  /// Creates an observer from the app-facing OpenAPI operation inventory.
  BackendLedgerInterceptor(Iterable<BackendOperation> operations)
    : _operations = List<BackendOperation>.unmodifiable(operations);

  final List<BackendOperation> _operations;
  final List<BackendObservation> _observations = [];

  /// The immutable observations collected so far.
  List<BackendObservation> get observations =>
      List<BackendObservation>.unmodifiable(_observations);

  @override
  void onResponse(
    Response<Object?> response,
    ResponseInterceptorHandler handler,
  ) {
    _record(response.requestOptions, response.statusCode);
    handler.next(response);
  }

  @override
  void onError(DioException err, ErrorInterceptorHandler handler) {
    _record(err.requestOptions, err.response?.statusCode);
    handler.next(err);
  }

  /// Throws unless every expected slice was observed with [outcome].
  void expectSlices(
    Iterable<String> operationIds, {
    required BackendOutcome outcome,
  }) {
    final missing = operationIds
        .where((id) {
          return !_observations.any(
            (item) =>
                item.operationId == id &&
                (outcome == BackendOutcome.success
                    ? item.isSuccess
                    : !item.isSuccess),
          );
        })
        .toList(growable: false);
    if (missing.isNotEmpty) {
      throw StateError(
        'Backend operations lacked ${outcome.name} evidence: ${missing.join(', ')}',
      );
    }
  }

  void _record(RequestOptions request, int? statusCode) {
    if (statusCode == null) return;
    final operation = _operations
        .where((item) => item._matches(request))
        .firstOrNull;
    if (operation == null) return;
    _observations.add(
      BackendObservation(
        operationId: operation.operationId,
        statusCode: statusCode,
      ),
    );
  }
}

RegExp _compilePath(String template) {
  final segments = template.split('/').map((segment) {
    if (segment.startsWith('{') && segment.endsWith('}')) return '[^/]+';
    return RegExp.escape(segment);
  });
  return RegExp('^${segments.join('/')}/?\$');
}
