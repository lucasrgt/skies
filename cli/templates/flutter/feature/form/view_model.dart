import 'package:flutter/foundation.dart';
import 'package:skies_flutter/skies_flutter.dart';

/// The form's fields in visual order: `submitOrReveal` reveals the first invalid one.
enum {{ feature }}Field { id }

/// The typed composition-root port that sends the command through its generated operation. The composition root
/// runs it inside the app's `MutationBoundary`, which invalidates cached reads and posts the global feedback.
typedef Send{{ feature }} = Future<void> Function({required String id});

/// Owns the form's values, its validation, and the command's state for [{{ feature }}View]. The fields start as the
/// `g slice` scaffold's Input (`Id`): replace them with the slice's real Input, and validate only the rules the
/// slice itself holds.
final class {{ feature }}ViewModel extends ChangeNotifier {
  {{ feature }}ViewModel({required this.send{{ feature }}});

  /// The generated-client wiring supplied by the composition root.
  final Send{{ feature }} send{{ feature }};

  String _id = '';
  Set<{{ feature }}Field> _invalid = const {};
  {{ feature }}Field? _revealed;
  AsyncState<bool> _submission = const AsyncEmpty<bool>();

  /// The command's closed state: empty until the first submit, then loading, ready once sent, or failure.
  AsyncState<bool> get submission => _submission;

  /// Whether the command is in flight; the submit shows it and ignores presses.
  bool get submitting => _submission is AsyncLoading<bool>;

  /// The success surface: a routed app redirects on it declaratively.
  bool get completed => _submission is AsyncReady<bool>;

  /// The command's failure, kept for the inline error surface next to the submit.
  Object? get failure => switch (_submission) {
        AsyncFailure<bool>(:final error) => error,
        _ => null,
      };

  /// The first invalid field of the last submit, for the View to focus or scroll to.
  {{ feature }}Field? get revealed => _revealed;

  /// Whether [field] failed validation on the last submit; its control shows the error copy.
  bool isInvalid({{ feature }}Field field) => _invalid.contains(field);

  /// The id field's value, as the control holds it.
  String get id => _id;

  /// Updates the id field and clears its error.
  void setId(String value) {
    _id = value.trim();
    _invalid = {..._invalid}..remove({{ feature }}Field.id);
    notifyListeners();
  }

  /// Validates, then either sends the command or reveals the first invalid field.
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
