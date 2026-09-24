import 'package:flutter/foundation.dart';
import 'package:skies_flutter/skies_flutter.dart';

/// One row of this feature. Replace it with the generated client's model once the slice exists.
final class {{ item }} {
  const {{ item }}({required this.id, required this.name});

  final String id;
  final String name;
}

/// The typed composition-root port that wires this feature to its generated operation.
typedef Load{{ feature }} = Future<List<{{ item }}>> Function();

/// Owns the complete UI state and commands for [{{ feature }}View].
final class {{ feature }}ViewModel extends ChangeNotifier {
  {{ feature }}ViewModel({required this.load{{ feature }}});

  /// The generated-client wiring supplied by the composition root.
  final Load{{ feature }} load{{ feature }};
  AsyncState<List<{{ item }}>> _state = const AsyncLoading<List<{{ item }}>>();

  /// The closed resource state consumed by the View.
  AsyncState<List<{{ item }}>> get state => _state;

  /// Loads or retries this feature through its typed generated-client wiring.
  Future<void> load() async {
    _state = const AsyncLoading<List<{{ item }}>>();
    notifyListeners();
    try {
      final items = List<{{ item }}>.unmodifiable(await load{{ feature }}());
      _state = items.isEmpty
          ? const AsyncEmpty<List<{{ item }}>>()
          : AsyncReady<List<{{ item }}>>(items);
    } on Object catch (error, stackTrace) {
      _state = AsyncFailure<List<{{ item }}>>(error, stackTrace);
    }
    notifyListeners();
  }
}
