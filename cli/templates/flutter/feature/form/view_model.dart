import 'package:flutter/foundation.dart';
import 'package:skies_flutter/skies_flutter.dart';

// Placeholder: `id` mirrors the `g slice` scaffold's Input. Replace it with the slice's real fields, and validate
// only the rules the slice itself holds.
enum {{ feature }}Field { id }

/// Sends the command through its generated operation; the composition root runs it inside the app's
/// `MutationBoundary`, which refreshes cached reads and posts the global feedback.
typedef Send{{ feature }} = Future<void> Function({required String id});

/// Owns the form's values, its validation, and the command's state for [{{ feature }}View].
final class {{ feature }}ViewModel extends ChangeNotifier {
  {{ feature }}ViewModel({required this.send{{ feature }}});

  final Send{{ feature }} send{{ feature }};

  String _id = '';
  Set<{{ feature }}Field> _invalid = const {};
  {{ feature }}Field? _revealed;
  AsyncState<bool> _submission = const AsyncEmpty<bool>();

  AsyncState<bool> get submission => _submission;
  bool get submitting => _submission is AsyncLoading<bool>;
  bool get completed => _submission is AsyncReady<bool>;

  Object? get failure => switch (_submission) {
        AsyncFailure<bool>(:final error) => error,
        _ => null,
      };

  /// The first invalid field of the last submit, for the View to focus or scroll to.
  {{ feature }}Field? get revealed => _revealed;

  bool isInvalid({{ feature }}Field field) => _invalid.contains(field);

  String get id => _id;

  void setId(String value) {
    _id = value.trim();
    _invalid = {..._invalid}..remove({{ feature }}Field.id);
    notifyListeners();
  }

  Future<void> submit() async {
    if (submitting) return;
    await submitOrReveal<{{ feature }}Field>(
      validate: _check,
      invalidFields: () => _invalid,
      onValid: _send,
      onInvalid: (first) {
        _revealed = first;
        notifyListeners();
      },
      order: {{ feature }}Field.values,
    );
  }

  bool _check() {
    _invalid = {if (!_uuid.hasMatch(_id)) {{ feature }}Field.id};
    _revealed = null;
    notifyListeners();
    return _invalid.isEmpty;
  }

  Future<void> _send() async {
    _submission = const AsyncLoading<bool>();
    notifyListeners();
    try {
      await send{{ feature }}(id: _id);
      _submission = const AsyncReady<bool>(true);
    } on Object catch (error, stackTrace) {
      _submission = AsyncFailure<bool>(error, stackTrace, retry: submit);
    }
    notifyListeners();
  }
}

final _uuid = RegExp(
  r'^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$',
);
