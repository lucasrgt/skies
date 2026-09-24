import 'package:flutter/widgets.dart';
import 'package:skies_flutter/skies_flutter.dart';

import '{{ stem }}_view_model.dart';

/// Routes the command's state to a surface: every state but a sent command shows the form.
final class {{ feature }}View extends StatelessWidget {
  const {{ feature }}View({
    required this.viewModel,
    required this.form,
    required this.done,
    super.key,
  });

  final {{ feature }}ViewModel viewModel;

  /// The fields (error copy on `isInvalid`), the inline `failure`, and the submit (`submitting`, `submit`).
  final Widget Function(BuildContext context, {{ feature }}ViewModel viewModel) form;

  /// The success surface; a routed app redirects instead.
  final WidgetBuilder done;

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
        animation: viewModel,
        builder: (context, _) => ResourceBuilder<bool>(
          state: viewModel.submission,
          loading: (context) => form(context, viewModel),
          empty: (context) => form(context, viewModel),
          failure: (context, _, _) => form(context, viewModel),
          ready: (context, _) => done(context),
        ),
      );
}
