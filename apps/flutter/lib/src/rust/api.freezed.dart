// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint, type=warning, deprecated_member_use, deprecated_member_use_from_same_package
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'api.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// GENERATED CODE - DO NOT MODIFY BY HAND
// dart format off
T _$identity<T>(T value) => value;

/// @nodoc
mixin _$ReceiverUiEvent {
  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType && other is ReceiverUiEvent);
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  String toString() {
    return 'ReceiverUiEvent()';
  }
}

/// @nodoc
class $ReceiverUiEventCopyWith<$Res> {
  $ReceiverUiEventCopyWith(
      ReceiverUiEvent _, $Res Function(ReceiverUiEvent) __);
}

/// Adds pattern-matching-related methods to [ReceiverUiEvent].
extension ReceiverUiEventPatterns on ReceiverUiEvent {
  /// A variant of `map` that fallback to returning `orElse`.
  ///
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case final Subclass value:
  ///     return ...;
  ///   case _:
  ///     return orElse();
  /// }
  /// ```

  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(ReceiverUiEvent_RoomCode value)? roomCode,
    TResult Function(ReceiverUiEvent_PeerJoined value)? peerJoined,
    TResult Function(ReceiverUiEvent_ConnectionState value)? connectionState,
    TResult Function(ReceiverUiEvent_VideoTrack value)? videoTrack,
    TResult Function(ReceiverUiEvent_Stats value)? stats,
    TResult Function(ReceiverUiEvent_PeerLeft value)? peerLeft,
    TResult Function(ReceiverUiEvent_Error value)? error,
    required TResult orElse(),
  }) {
    final _that = this;
    switch (_that) {
      case ReceiverUiEvent_RoomCode() when roomCode != null:
        return roomCode(_that);
      case ReceiverUiEvent_PeerJoined() when peerJoined != null:
        return peerJoined(_that);
      case ReceiverUiEvent_ConnectionState() when connectionState != null:
        return connectionState(_that);
      case ReceiverUiEvent_VideoTrack() when videoTrack != null:
        return videoTrack(_that);
      case ReceiverUiEvent_Stats() when stats != null:
        return stats(_that);
      case ReceiverUiEvent_PeerLeft() when peerLeft != null:
        return peerLeft(_that);
      case ReceiverUiEvent_Error() when error != null:
        return error(_that);
      case _:
        return orElse();
    }
  }

  /// A `switch`-like method, using callbacks.
  ///
  /// Callbacks receives the raw object, upcasted.
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case final Subclass value:
  ///     return ...;
  ///   case final Subclass2 value:
  ///     return ...;
  /// }
  /// ```

  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(ReceiverUiEvent_RoomCode value) roomCode,
    required TResult Function(ReceiverUiEvent_PeerJoined value) peerJoined,
    required TResult Function(ReceiverUiEvent_ConnectionState value)
        connectionState,
    required TResult Function(ReceiverUiEvent_VideoTrack value) videoTrack,
    required TResult Function(ReceiverUiEvent_Stats value) stats,
    required TResult Function(ReceiverUiEvent_PeerLeft value) peerLeft,
    required TResult Function(ReceiverUiEvent_Error value) error,
  }) {
    final _that = this;
    switch (_that) {
      case ReceiverUiEvent_RoomCode():
        return roomCode(_that);
      case ReceiverUiEvent_PeerJoined():
        return peerJoined(_that);
      case ReceiverUiEvent_ConnectionState():
        return connectionState(_that);
      case ReceiverUiEvent_VideoTrack():
        return videoTrack(_that);
      case ReceiverUiEvent_Stats():
        return stats(_that);
      case ReceiverUiEvent_PeerLeft():
        return peerLeft(_that);
      case ReceiverUiEvent_Error():
        return error(_that);
    }
  }

  /// A variant of `map` that fallback to returning `null`.
  ///
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case final Subclass value:
  ///     return ...;
  ///   case _:
  ///     return null;
  /// }
  /// ```

  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(ReceiverUiEvent_RoomCode value)? roomCode,
    TResult? Function(ReceiverUiEvent_PeerJoined value)? peerJoined,
    TResult? Function(ReceiverUiEvent_ConnectionState value)? connectionState,
    TResult? Function(ReceiverUiEvent_VideoTrack value)? videoTrack,
    TResult? Function(ReceiverUiEvent_Stats value)? stats,
    TResult? Function(ReceiverUiEvent_PeerLeft value)? peerLeft,
    TResult? Function(ReceiverUiEvent_Error value)? error,
  }) {
    final _that = this;
    switch (_that) {
      case ReceiverUiEvent_RoomCode() when roomCode != null:
        return roomCode(_that);
      case ReceiverUiEvent_PeerJoined() when peerJoined != null:
        return peerJoined(_that);
      case ReceiverUiEvent_ConnectionState() when connectionState != null:
        return connectionState(_that);
      case ReceiverUiEvent_VideoTrack() when videoTrack != null:
        return videoTrack(_that);
      case ReceiverUiEvent_Stats() when stats != null:
        return stats(_that);
      case ReceiverUiEvent_PeerLeft() when peerLeft != null:
        return peerLeft(_that);
      case ReceiverUiEvent_Error() when error != null:
        return error(_that);
      case _:
        return null;
    }
  }

  /// A variant of `when` that fallback to an `orElse` callback.
  ///
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case Subclass(:final field):
  ///     return ...;
  ///   case _:
  ///     return orElse();
  /// }
  /// ```

  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String code)? roomCode,
    TResult Function()? peerJoined,
    TResult Function(UiConnectionState state)? connectionState,
    TResult Function(String mime, String? codec)? videoTrack,
    TResult Function(
            BigInt packets, BigInt bytes, BigInt frames, BigInt keyframes)?
        stats,
    TResult Function()? peerLeft,
    TResult Function(String message)? error,
    required TResult orElse(),
  }) {
    final _that = this;
    switch (_that) {
      case ReceiverUiEvent_RoomCode() when roomCode != null:
        return roomCode(_that.code);
      case ReceiverUiEvent_PeerJoined() when peerJoined != null:
        return peerJoined();
      case ReceiverUiEvent_ConnectionState() when connectionState != null:
        return connectionState(_that.state);
      case ReceiverUiEvent_VideoTrack() when videoTrack != null:
        return videoTrack(_that.mime, _that.codec);
      case ReceiverUiEvent_Stats() when stats != null:
        return stats(_that.packets, _that.bytes, _that.frames, _that.keyframes);
      case ReceiverUiEvent_PeerLeft() when peerLeft != null:
        return peerLeft();
      case ReceiverUiEvent_Error() when error != null:
        return error(_that.message);
      case _:
        return orElse();
    }
  }

  /// A `switch`-like method, using callbacks.
  ///
  /// As opposed to `map`, this offers destructuring.
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case Subclass(:final field):
  ///     return ...;
  ///   case Subclass2(:final field2):
  ///     return ...;
  /// }
  /// ```

  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String code) roomCode,
    required TResult Function() peerJoined,
    required TResult Function(UiConnectionState state) connectionState,
    required TResult Function(String mime, String? codec) videoTrack,
    required TResult Function(
            BigInt packets, BigInt bytes, BigInt frames, BigInt keyframes)
        stats,
    required TResult Function() peerLeft,
    required TResult Function(String message) error,
  }) {
    final _that = this;
    switch (_that) {
      case ReceiverUiEvent_RoomCode():
        return roomCode(_that.code);
      case ReceiverUiEvent_PeerJoined():
        return peerJoined();
      case ReceiverUiEvent_ConnectionState():
        return connectionState(_that.state);
      case ReceiverUiEvent_VideoTrack():
        return videoTrack(_that.mime, _that.codec);
      case ReceiverUiEvent_Stats():
        return stats(_that.packets, _that.bytes, _that.frames, _that.keyframes);
      case ReceiverUiEvent_PeerLeft():
        return peerLeft();
      case ReceiverUiEvent_Error():
        return error(_that.message);
    }
  }

  /// A variant of `when` that fallback to returning `null`
  ///
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case Subclass(:final field):
  ///     return ...;
  ///   case _:
  ///     return null;
  /// }
  /// ```

  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String code)? roomCode,
    TResult? Function()? peerJoined,
    TResult? Function(UiConnectionState state)? connectionState,
    TResult? Function(String mime, String? codec)? videoTrack,
    TResult? Function(
            BigInt packets, BigInt bytes, BigInt frames, BigInt keyframes)?
        stats,
    TResult? Function()? peerLeft,
    TResult? Function(String message)? error,
  }) {
    final _that = this;
    switch (_that) {
      case ReceiverUiEvent_RoomCode() when roomCode != null:
        return roomCode(_that.code);
      case ReceiverUiEvent_PeerJoined() when peerJoined != null:
        return peerJoined();
      case ReceiverUiEvent_ConnectionState() when connectionState != null:
        return connectionState(_that.state);
      case ReceiverUiEvent_VideoTrack() when videoTrack != null:
        return videoTrack(_that.mime, _that.codec);
      case ReceiverUiEvent_Stats() when stats != null:
        return stats(_that.packets, _that.bytes, _that.frames, _that.keyframes);
      case ReceiverUiEvent_PeerLeft() when peerLeft != null:
        return peerLeft();
      case ReceiverUiEvent_Error() when error != null:
        return error(_that.message);
      case _:
        return null;
    }
  }
}

/// @nodoc

class ReceiverUiEvent_RoomCode extends ReceiverUiEvent {
  const ReceiverUiEvent_RoomCode({required this.code}) : super._();

  /// The room code, uppercase.
  final String code;

  /// Create a copy of ReceiverUiEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $ReceiverUiEvent_RoomCodeCopyWith<ReceiverUiEvent_RoomCode> get copyWith =>
      _$ReceiverUiEvent_RoomCodeCopyWithImpl<ReceiverUiEvent_RoomCode>(
          this, _$identity);

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is ReceiverUiEvent_RoomCode &&
            (identical(other.code, code) || other.code == code));
  }

  @override
  int get hashCode {
    return Object.hash(runtimeType, code);
  }

  @override
  String toString() {
    return 'ReceiverUiEvent.roomCode(code: $code)';
  }
}

/// @nodoc
abstract mixin class $ReceiverUiEvent_RoomCodeCopyWith<$Res>
    implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_RoomCodeCopyWith(ReceiverUiEvent_RoomCode value,
          $Res Function(ReceiverUiEvent_RoomCode) _then) =
      _$ReceiverUiEvent_RoomCodeCopyWithImpl;
  @useResult
  $Res call({String code});
}

/// @nodoc
class _$ReceiverUiEvent_RoomCodeCopyWithImpl<$Res>
    implements $ReceiverUiEvent_RoomCodeCopyWith<$Res> {
  _$ReceiverUiEvent_RoomCodeCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_RoomCode _self;
  final $Res Function(ReceiverUiEvent_RoomCode) _then;

  /// Create a copy of ReceiverUiEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  $Res call({
    Object? code = null,
  }) {
    return _then(ReceiverUiEvent_RoomCode(
      code: null == code
          ? _self.code
          : code // ignore: cast_nullable_to_non_nullable
              as String,
    ));
  }
}

/// @nodoc

class ReceiverUiEvent_PeerJoined extends ReceiverUiEvent {
  const ReceiverUiEvent_PeerJoined() : super._();

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is ReceiverUiEvent_PeerJoined);
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  String toString() {
    return 'ReceiverUiEvent.peerJoined()';
  }
}

/// @nodoc

class ReceiverUiEvent_ConnectionState extends ReceiverUiEvent {
  const ReceiverUiEvent_ConnectionState({required this.state}) : super._();

  /// New state.
  final UiConnectionState state;

  /// Create a copy of ReceiverUiEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $ReceiverUiEvent_ConnectionStateCopyWith<ReceiverUiEvent_ConnectionState>
      get copyWith => _$ReceiverUiEvent_ConnectionStateCopyWithImpl<
          ReceiverUiEvent_ConnectionState>(this, _$identity);

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is ReceiverUiEvent_ConnectionState &&
            (identical(other.state, state) || other.state == state));
  }

  @override
  int get hashCode {
    return Object.hash(runtimeType, state);
  }

  @override
  String toString() {
    return 'ReceiverUiEvent.connectionState(state: $state)';
  }
}

/// @nodoc
abstract mixin class $ReceiverUiEvent_ConnectionStateCopyWith<$Res>
    implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_ConnectionStateCopyWith(
          ReceiverUiEvent_ConnectionState value,
          $Res Function(ReceiverUiEvent_ConnectionState) _then) =
      _$ReceiverUiEvent_ConnectionStateCopyWithImpl;
  @useResult
  $Res call({UiConnectionState state});
}

/// @nodoc
class _$ReceiverUiEvent_ConnectionStateCopyWithImpl<$Res>
    implements $ReceiverUiEvent_ConnectionStateCopyWith<$Res> {
  _$ReceiverUiEvent_ConnectionStateCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_ConnectionState _self;
  final $Res Function(ReceiverUiEvent_ConnectionState) _then;

  /// Create a copy of ReceiverUiEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  $Res call({
    Object? state = null,
  }) {
    return _then(ReceiverUiEvent_ConnectionState(
      state: null == state
          ? _self.state
          : state // ignore: cast_nullable_to_non_nullable
              as UiConnectionState,
    ));
  }
}

/// @nodoc

class ReceiverUiEvent_VideoTrack extends ReceiverUiEvent {
  const ReceiverUiEvent_VideoTrack({required this.mime, this.codec})
      : super._();

  /// MIME type, e.g. `video/VP8`.
  final String mime;

  /// Codec name (`vp8`, `vp9`, `h264`, `av1`) or `None` if unknown.
  final String? codec;

  /// Create a copy of ReceiverUiEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $ReceiverUiEvent_VideoTrackCopyWith<ReceiverUiEvent_VideoTrack>
      get copyWith =>
          _$ReceiverUiEvent_VideoTrackCopyWithImpl<ReceiverUiEvent_VideoTrack>(
              this, _$identity);

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is ReceiverUiEvent_VideoTrack &&
            (identical(other.mime, mime) || other.mime == mime) &&
            (identical(other.codec, codec) || other.codec == codec));
  }

  @override
  int get hashCode {
    return Object.hash(runtimeType, mime, codec);
  }

  @override
  String toString() {
    return 'ReceiverUiEvent.videoTrack(mime: $mime, codec: $codec)';
  }
}

/// @nodoc
abstract mixin class $ReceiverUiEvent_VideoTrackCopyWith<$Res>
    implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_VideoTrackCopyWith(ReceiverUiEvent_VideoTrack value,
          $Res Function(ReceiverUiEvent_VideoTrack) _then) =
      _$ReceiverUiEvent_VideoTrackCopyWithImpl;
  @useResult
  $Res call({String mime, String? codec});
}

/// @nodoc
class _$ReceiverUiEvent_VideoTrackCopyWithImpl<$Res>
    implements $ReceiverUiEvent_VideoTrackCopyWith<$Res> {
  _$ReceiverUiEvent_VideoTrackCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_VideoTrack _self;
  final $Res Function(ReceiverUiEvent_VideoTrack) _then;

  /// Create a copy of ReceiverUiEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  $Res call({
    Object? mime = null,
    Object? codec = freezed,
  }) {
    return _then(ReceiverUiEvent_VideoTrack(
      mime: null == mime
          ? _self.mime
          : mime // ignore: cast_nullable_to_non_nullable
              as String,
      codec: freezed == codec
          ? _self.codec
          : codec // ignore: cast_nullable_to_non_nullable
              as String?,
    ));
  }
}

/// @nodoc

class ReceiverUiEvent_Stats extends ReceiverUiEvent {
  const ReceiverUiEvent_Stats(
      {required this.packets,
      required this.bytes,
      required this.frames,
      required this.keyframes})
      : super._();

  /// RTP packets received.
  final BigInt packets;

  /// RTP payload bytes received.
  final BigInt bytes;

  /// Frames received.
  final BigInt frames;

  /// Key frames received.
  final BigInt keyframes;

  /// Create a copy of ReceiverUiEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $ReceiverUiEvent_StatsCopyWith<ReceiverUiEvent_Stats> get copyWith =>
      _$ReceiverUiEvent_StatsCopyWithImpl<ReceiverUiEvent_Stats>(
          this, _$identity);

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is ReceiverUiEvent_Stats &&
            (identical(other.packets, packets) || other.packets == packets) &&
            (identical(other.bytes, bytes) || other.bytes == bytes) &&
            (identical(other.frames, frames) || other.frames == frames) &&
            (identical(other.keyframes, keyframes) ||
                other.keyframes == keyframes));
  }

  @override
  int get hashCode {
    return Object.hash(runtimeType, packets, bytes, frames, keyframes);
  }

  @override
  String toString() {
    return 'ReceiverUiEvent.stats(packets: $packets, bytes: $bytes, frames: $frames, keyframes: $keyframes)';
  }
}

/// @nodoc
abstract mixin class $ReceiverUiEvent_StatsCopyWith<$Res>
    implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_StatsCopyWith(ReceiverUiEvent_Stats value,
          $Res Function(ReceiverUiEvent_Stats) _then) =
      _$ReceiverUiEvent_StatsCopyWithImpl;
  @useResult
  $Res call({BigInt packets, BigInt bytes, BigInt frames, BigInt keyframes});
}

/// @nodoc
class _$ReceiverUiEvent_StatsCopyWithImpl<$Res>
    implements $ReceiverUiEvent_StatsCopyWith<$Res> {
  _$ReceiverUiEvent_StatsCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_Stats _self;
  final $Res Function(ReceiverUiEvent_Stats) _then;

  /// Create a copy of ReceiverUiEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  $Res call({
    Object? packets = null,
    Object? bytes = null,
    Object? frames = null,
    Object? keyframes = null,
  }) {
    return _then(ReceiverUiEvent_Stats(
      packets: null == packets
          ? _self.packets
          : packets // ignore: cast_nullable_to_non_nullable
              as BigInt,
      bytes: null == bytes
          ? _self.bytes
          : bytes // ignore: cast_nullable_to_non_nullable
              as BigInt,
      frames: null == frames
          ? _self.frames
          : frames // ignore: cast_nullable_to_non_nullable
              as BigInt,
      keyframes: null == keyframes
          ? _self.keyframes
          : keyframes // ignore: cast_nullable_to_non_nullable
              as BigInt,
    ));
  }
}

/// @nodoc

class ReceiverUiEvent_PeerLeft extends ReceiverUiEvent {
  const ReceiverUiEvent_PeerLeft() : super._();

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType && other is ReceiverUiEvent_PeerLeft);
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  String toString() {
    return 'ReceiverUiEvent.peerLeft()';
  }
}

/// @nodoc

class ReceiverUiEvent_Error extends ReceiverUiEvent {
  const ReceiverUiEvent_Error({required this.message}) : super._();

  /// Technical message in English (for logs and for showing next to a translated title).
  final String message;

  /// Create a copy of ReceiverUiEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $ReceiverUiEvent_ErrorCopyWith<ReceiverUiEvent_Error> get copyWith =>
      _$ReceiverUiEvent_ErrorCopyWithImpl<ReceiverUiEvent_Error>(
          this, _$identity);

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is ReceiverUiEvent_Error &&
            (identical(other.message, message) || other.message == message));
  }

  @override
  int get hashCode {
    return Object.hash(runtimeType, message);
  }

  @override
  String toString() {
    return 'ReceiverUiEvent.error(message: $message)';
  }
}

/// @nodoc
abstract mixin class $ReceiverUiEvent_ErrorCopyWith<$Res>
    implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_ErrorCopyWith(ReceiverUiEvent_Error value,
          $Res Function(ReceiverUiEvent_Error) _then) =
      _$ReceiverUiEvent_ErrorCopyWithImpl;
  @useResult
  $Res call({String message});
}

/// @nodoc
class _$ReceiverUiEvent_ErrorCopyWithImpl<$Res>
    implements $ReceiverUiEvent_ErrorCopyWith<$Res> {
  _$ReceiverUiEvent_ErrorCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_Error _self;
  final $Res Function(ReceiverUiEvent_Error) _then;

  /// Create a copy of ReceiverUiEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  $Res call({
    Object? message = null,
  }) {
    return _then(ReceiverUiEvent_Error(
      message: null == message
          ? _self.message
          : message // ignore: cast_nullable_to_non_nullable
              as String,
    ));
  }
}

/// @nodoc
mixin _$RoomCodeCheck {
  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType && other is RoomCodeCheck);
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  String toString() {
    return 'RoomCodeCheck()';
  }
}

/// @nodoc
class $RoomCodeCheckCopyWith<$Res> {
  $RoomCodeCheckCopyWith(RoomCodeCheck _, $Res Function(RoomCodeCheck) __);
}

/// Adds pattern-matching-related methods to [RoomCodeCheck].
extension RoomCodeCheckPatterns on RoomCodeCheck {
  /// A variant of `map` that fallback to returning `orElse`.
  ///
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case final Subclass value:
  ///     return ...;
  ///   case _:
  ///     return orElse();
  /// }
  /// ```

  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(RoomCodeCheck_Valid value)? valid,
    TResult Function(RoomCodeCheck_WrongLength value)? wrongLength,
    TResult Function(RoomCodeCheck_InvalidChar value)? invalidChar,
    required TResult orElse(),
  }) {
    final _that = this;
    switch (_that) {
      case RoomCodeCheck_Valid() when valid != null:
        return valid(_that);
      case RoomCodeCheck_WrongLength() when wrongLength != null:
        return wrongLength(_that);
      case RoomCodeCheck_InvalidChar() when invalidChar != null:
        return invalidChar(_that);
      case _:
        return orElse();
    }
  }

  /// A `switch`-like method, using callbacks.
  ///
  /// Callbacks receives the raw object, upcasted.
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case final Subclass value:
  ///     return ...;
  ///   case final Subclass2 value:
  ///     return ...;
  /// }
  /// ```

  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(RoomCodeCheck_Valid value) valid,
    required TResult Function(RoomCodeCheck_WrongLength value) wrongLength,
    required TResult Function(RoomCodeCheck_InvalidChar value) invalidChar,
  }) {
    final _that = this;
    switch (_that) {
      case RoomCodeCheck_Valid():
        return valid(_that);
      case RoomCodeCheck_WrongLength():
        return wrongLength(_that);
      case RoomCodeCheck_InvalidChar():
        return invalidChar(_that);
    }
  }

  /// A variant of `map` that fallback to returning `null`.
  ///
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case final Subclass value:
  ///     return ...;
  ///   case _:
  ///     return null;
  /// }
  /// ```

  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(RoomCodeCheck_Valid value)? valid,
    TResult? Function(RoomCodeCheck_WrongLength value)? wrongLength,
    TResult? Function(RoomCodeCheck_InvalidChar value)? invalidChar,
  }) {
    final _that = this;
    switch (_that) {
      case RoomCodeCheck_Valid() when valid != null:
        return valid(_that);
      case RoomCodeCheck_WrongLength() when wrongLength != null:
        return wrongLength(_that);
      case RoomCodeCheck_InvalidChar() when invalidChar != null:
        return invalidChar(_that);
      case _:
        return null;
    }
  }

  /// A variant of `when` that fallback to an `orElse` callback.
  ///
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case Subclass(:final field):
  ///     return ...;
  ///   case _:
  ///     return orElse();
  /// }
  /// ```

  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String code)? valid,
    TResult Function()? wrongLength,
    TResult Function(String ch)? invalidChar,
    required TResult orElse(),
  }) {
    final _that = this;
    switch (_that) {
      case RoomCodeCheck_Valid() when valid != null:
        return valid(_that.code);
      case RoomCodeCheck_WrongLength() when wrongLength != null:
        return wrongLength();
      case RoomCodeCheck_InvalidChar() when invalidChar != null:
        return invalidChar(_that.ch);
      case _:
        return orElse();
    }
  }

  /// A `switch`-like method, using callbacks.
  ///
  /// As opposed to `map`, this offers destructuring.
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case Subclass(:final field):
  ///     return ...;
  ///   case Subclass2(:final field2):
  ///     return ...;
  /// }
  /// ```

  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String code) valid,
    required TResult Function() wrongLength,
    required TResult Function(String ch) invalidChar,
  }) {
    final _that = this;
    switch (_that) {
      case RoomCodeCheck_Valid():
        return valid(_that.code);
      case RoomCodeCheck_WrongLength():
        return wrongLength();
      case RoomCodeCheck_InvalidChar():
        return invalidChar(_that.ch);
    }
  }

  /// A variant of `when` that fallback to returning `null`
  ///
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case Subclass(:final field):
  ///     return ...;
  ///   case _:
  ///     return null;
  /// }
  /// ```

  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String code)? valid,
    TResult? Function()? wrongLength,
    TResult? Function(String ch)? invalidChar,
  }) {
    final _that = this;
    switch (_that) {
      case RoomCodeCheck_Valid() when valid != null:
        return valid(_that.code);
      case RoomCodeCheck_WrongLength() when wrongLength != null:
        return wrongLength();
      case RoomCodeCheck_InvalidChar() when invalidChar != null:
        return invalidChar(_that.ch);
      case _:
        return null;
    }
  }
}

/// @nodoc

class RoomCodeCheck_Valid extends RoomCodeCheck {
  const RoomCodeCheck_Valid({required this.code}) : super._();

  /// The normalised room code.
  final String code;

  /// Create a copy of RoomCodeCheck
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $RoomCodeCheck_ValidCopyWith<RoomCodeCheck_Valid> get copyWith =>
      _$RoomCodeCheck_ValidCopyWithImpl<RoomCodeCheck_Valid>(this, _$identity);

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is RoomCodeCheck_Valid &&
            (identical(other.code, code) || other.code == code));
  }

  @override
  int get hashCode {
    return Object.hash(runtimeType, code);
  }

  @override
  String toString() {
    return 'RoomCodeCheck.valid(code: $code)';
  }
}

/// @nodoc
abstract mixin class $RoomCodeCheck_ValidCopyWith<$Res>
    implements $RoomCodeCheckCopyWith<$Res> {
  factory $RoomCodeCheck_ValidCopyWith(
          RoomCodeCheck_Valid value, $Res Function(RoomCodeCheck_Valid) _then) =
      _$RoomCodeCheck_ValidCopyWithImpl;
  @useResult
  $Res call({String code});
}

/// @nodoc
class _$RoomCodeCheck_ValidCopyWithImpl<$Res>
    implements $RoomCodeCheck_ValidCopyWith<$Res> {
  _$RoomCodeCheck_ValidCopyWithImpl(this._self, this._then);

  final RoomCodeCheck_Valid _self;
  final $Res Function(RoomCodeCheck_Valid) _then;

  /// Create a copy of RoomCodeCheck
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  $Res call({
    Object? code = null,
  }) {
    return _then(RoomCodeCheck_Valid(
      code: null == code
          ? _self.code
          : code // ignore: cast_nullable_to_non_nullable
              as String,
    ));
  }
}

/// @nodoc

class RoomCodeCheck_WrongLength extends RoomCodeCheck {
  const RoomCodeCheck_WrongLength() : super._();

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is RoomCodeCheck_WrongLength);
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  String toString() {
    return 'RoomCodeCheck.wrongLength()';
  }
}

/// @nodoc

class RoomCodeCheck_InvalidChar extends RoomCodeCheck {
  const RoomCodeCheck_InvalidChar({required this.ch}) : super._();

  /// The first offending character.
  final String ch;

  /// Create a copy of RoomCodeCheck
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $RoomCodeCheck_InvalidCharCopyWith<RoomCodeCheck_InvalidChar> get copyWith =>
      _$RoomCodeCheck_InvalidCharCopyWithImpl<RoomCodeCheck_InvalidChar>(
          this, _$identity);

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is RoomCodeCheck_InvalidChar &&
            (identical(other.ch, ch) || other.ch == ch));
  }

  @override
  int get hashCode {
    return Object.hash(runtimeType, ch);
  }

  @override
  String toString() {
    return 'RoomCodeCheck.invalidChar(ch: $ch)';
  }
}

/// @nodoc
abstract mixin class $RoomCodeCheck_InvalidCharCopyWith<$Res>
    implements $RoomCodeCheckCopyWith<$Res> {
  factory $RoomCodeCheck_InvalidCharCopyWith(RoomCodeCheck_InvalidChar value,
          $Res Function(RoomCodeCheck_InvalidChar) _then) =
      _$RoomCodeCheck_InvalidCharCopyWithImpl;
  @useResult
  $Res call({String ch});
}

/// @nodoc
class _$RoomCodeCheck_InvalidCharCopyWithImpl<$Res>
    implements $RoomCodeCheck_InvalidCharCopyWith<$Res> {
  _$RoomCodeCheck_InvalidCharCopyWithImpl(this._self, this._then);

  final RoomCodeCheck_InvalidChar _self;
  final $Res Function(RoomCodeCheck_InvalidChar) _then;

  /// Create a copy of RoomCodeCheck
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  $Res call({
    Object? ch = null,
  }) {
    return _then(RoomCodeCheck_InvalidChar(
      ch: null == ch
          ? _self.ch
          : ch // ignore: cast_nullable_to_non_nullable
              as String,
    ));
  }
}

/// @nodoc
mixin _$SenderUiEvent {
  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType && other is SenderUiEvent);
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  String toString() {
    return 'SenderUiEvent()';
  }
}

/// @nodoc
class $SenderUiEventCopyWith<$Res> {
  $SenderUiEventCopyWith(SenderUiEvent _, $Res Function(SenderUiEvent) __);
}

/// Adds pattern-matching-related methods to [SenderUiEvent].
extension SenderUiEventPatterns on SenderUiEvent {
  /// A variant of `map` that fallback to returning `orElse`.
  ///
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case final Subclass value:
  ///     return ...;
  ///   case _:
  ///     return orElse();
  /// }
  /// ```

  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(SenderUiEvent_Joined value)? joined,
    TResult Function(SenderUiEvent_ConnectionState value)? connectionState,
    TResult Function(SenderUiEvent_Streaming value)? streaming,
    TResult Function(SenderUiEvent_Finished value)? finished,
    TResult Function(SenderUiEvent_PeerLeft value)? peerLeft,
    TResult Function(SenderUiEvent_Error value)? error,
    required TResult orElse(),
  }) {
    final _that = this;
    switch (_that) {
      case SenderUiEvent_Joined() when joined != null:
        return joined(_that);
      case SenderUiEvent_ConnectionState() when connectionState != null:
        return connectionState(_that);
      case SenderUiEvent_Streaming() when streaming != null:
        return streaming(_that);
      case SenderUiEvent_Finished() when finished != null:
        return finished(_that);
      case SenderUiEvent_PeerLeft() when peerLeft != null:
        return peerLeft(_that);
      case SenderUiEvent_Error() when error != null:
        return error(_that);
      case _:
        return orElse();
    }
  }

  /// A `switch`-like method, using callbacks.
  ///
  /// Callbacks receives the raw object, upcasted.
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case final Subclass value:
  ///     return ...;
  ///   case final Subclass2 value:
  ///     return ...;
  /// }
  /// ```

  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(SenderUiEvent_Joined value) joined,
    required TResult Function(SenderUiEvent_ConnectionState value)
        connectionState,
    required TResult Function(SenderUiEvent_Streaming value) streaming,
    required TResult Function(SenderUiEvent_Finished value) finished,
    required TResult Function(SenderUiEvent_PeerLeft value) peerLeft,
    required TResult Function(SenderUiEvent_Error value) error,
  }) {
    final _that = this;
    switch (_that) {
      case SenderUiEvent_Joined():
        return joined(_that);
      case SenderUiEvent_ConnectionState():
        return connectionState(_that);
      case SenderUiEvent_Streaming():
        return streaming(_that);
      case SenderUiEvent_Finished():
        return finished(_that);
      case SenderUiEvent_PeerLeft():
        return peerLeft(_that);
      case SenderUiEvent_Error():
        return error(_that);
    }
  }

  /// A variant of `map` that fallback to returning `null`.
  ///
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case final Subclass value:
  ///     return ...;
  ///   case _:
  ///     return null;
  /// }
  /// ```

  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(SenderUiEvent_Joined value)? joined,
    TResult? Function(SenderUiEvent_ConnectionState value)? connectionState,
    TResult? Function(SenderUiEvent_Streaming value)? streaming,
    TResult? Function(SenderUiEvent_Finished value)? finished,
    TResult? Function(SenderUiEvent_PeerLeft value)? peerLeft,
    TResult? Function(SenderUiEvent_Error value)? error,
  }) {
    final _that = this;
    switch (_that) {
      case SenderUiEvent_Joined() when joined != null:
        return joined(_that);
      case SenderUiEvent_ConnectionState() when connectionState != null:
        return connectionState(_that);
      case SenderUiEvent_Streaming() when streaming != null:
        return streaming(_that);
      case SenderUiEvent_Finished() when finished != null:
        return finished(_that);
      case SenderUiEvent_PeerLeft() when peerLeft != null:
        return peerLeft(_that);
      case SenderUiEvent_Error() when error != null:
        return error(_that);
      case _:
        return null;
    }
  }

  /// A variant of `when` that fallback to an `orElse` callback.
  ///
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case Subclass(:final field):
  ///     return ...;
  ///   case _:
  ///     return orElse();
  /// }
  /// ```

  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function()? joined,
    TResult Function(UiConnectionState state)? connectionState,
    TResult Function(BigInt framesSent)? streaming,
    TResult Function(BigInt framesSent)? finished,
    TResult Function()? peerLeft,
    TResult Function(String message)? error,
    required TResult orElse(),
  }) {
    final _that = this;
    switch (_that) {
      case SenderUiEvent_Joined() when joined != null:
        return joined();
      case SenderUiEvent_ConnectionState() when connectionState != null:
        return connectionState(_that.state);
      case SenderUiEvent_Streaming() when streaming != null:
        return streaming(_that.framesSent);
      case SenderUiEvent_Finished() when finished != null:
        return finished(_that.framesSent);
      case SenderUiEvent_PeerLeft() when peerLeft != null:
        return peerLeft();
      case SenderUiEvent_Error() when error != null:
        return error(_that.message);
      case _:
        return orElse();
    }
  }

  /// A `switch`-like method, using callbacks.
  ///
  /// As opposed to `map`, this offers destructuring.
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case Subclass(:final field):
  ///     return ...;
  ///   case Subclass2(:final field2):
  ///     return ...;
  /// }
  /// ```

  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function() joined,
    required TResult Function(UiConnectionState state) connectionState,
    required TResult Function(BigInt framesSent) streaming,
    required TResult Function(BigInt framesSent) finished,
    required TResult Function() peerLeft,
    required TResult Function(String message) error,
  }) {
    final _that = this;
    switch (_that) {
      case SenderUiEvent_Joined():
        return joined();
      case SenderUiEvent_ConnectionState():
        return connectionState(_that.state);
      case SenderUiEvent_Streaming():
        return streaming(_that.framesSent);
      case SenderUiEvent_Finished():
        return finished(_that.framesSent);
      case SenderUiEvent_PeerLeft():
        return peerLeft();
      case SenderUiEvent_Error():
        return error(_that.message);
    }
  }

  /// A variant of `when` that fallback to returning `null`
  ///
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case Subclass(:final field):
  ///     return ...;
  ///   case _:
  ///     return null;
  /// }
  /// ```

  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function()? joined,
    TResult? Function(UiConnectionState state)? connectionState,
    TResult? Function(BigInt framesSent)? streaming,
    TResult? Function(BigInt framesSent)? finished,
    TResult? Function()? peerLeft,
    TResult? Function(String message)? error,
  }) {
    final _that = this;
    switch (_that) {
      case SenderUiEvent_Joined() when joined != null:
        return joined();
      case SenderUiEvent_ConnectionState() when connectionState != null:
        return connectionState(_that.state);
      case SenderUiEvent_Streaming() when streaming != null:
        return streaming(_that.framesSent);
      case SenderUiEvent_Finished() when finished != null:
        return finished(_that.framesSent);
      case SenderUiEvent_PeerLeft() when peerLeft != null:
        return peerLeft();
      case SenderUiEvent_Error() when error != null:
        return error(_that.message);
      case _:
        return null;
    }
  }
}

/// @nodoc

class SenderUiEvent_Joined extends SenderUiEvent {
  const SenderUiEvent_Joined() : super._();

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType && other is SenderUiEvent_Joined);
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  String toString() {
    return 'SenderUiEvent.joined()';
  }
}

/// @nodoc

class SenderUiEvent_ConnectionState extends SenderUiEvent {
  const SenderUiEvent_ConnectionState({required this.state}) : super._();

  /// New state.
  final UiConnectionState state;

  /// Create a copy of SenderUiEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $SenderUiEvent_ConnectionStateCopyWith<SenderUiEvent_ConnectionState>
      get copyWith => _$SenderUiEvent_ConnectionStateCopyWithImpl<
          SenderUiEvent_ConnectionState>(this, _$identity);

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is SenderUiEvent_ConnectionState &&
            (identical(other.state, state) || other.state == state));
  }

  @override
  int get hashCode {
    return Object.hash(runtimeType, state);
  }

  @override
  String toString() {
    return 'SenderUiEvent.connectionState(state: $state)';
  }
}

/// @nodoc
abstract mixin class $SenderUiEvent_ConnectionStateCopyWith<$Res>
    implements $SenderUiEventCopyWith<$Res> {
  factory $SenderUiEvent_ConnectionStateCopyWith(
          SenderUiEvent_ConnectionState value,
          $Res Function(SenderUiEvent_ConnectionState) _then) =
      _$SenderUiEvent_ConnectionStateCopyWithImpl;
  @useResult
  $Res call({UiConnectionState state});
}

/// @nodoc
class _$SenderUiEvent_ConnectionStateCopyWithImpl<$Res>
    implements $SenderUiEvent_ConnectionStateCopyWith<$Res> {
  _$SenderUiEvent_ConnectionStateCopyWithImpl(this._self, this._then);

  final SenderUiEvent_ConnectionState _self;
  final $Res Function(SenderUiEvent_ConnectionState) _then;

  /// Create a copy of SenderUiEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  $Res call({
    Object? state = null,
  }) {
    return _then(SenderUiEvent_ConnectionState(
      state: null == state
          ? _self.state
          : state // ignore: cast_nullable_to_non_nullable
              as UiConnectionState,
    ));
  }
}

/// @nodoc

class SenderUiEvent_Streaming extends SenderUiEvent {
  const SenderUiEvent_Streaming({required this.framesSent}) : super._();

  /// Frames written to the track.
  final BigInt framesSent;

  /// Create a copy of SenderUiEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $SenderUiEvent_StreamingCopyWith<SenderUiEvent_Streaming> get copyWith =>
      _$SenderUiEvent_StreamingCopyWithImpl<SenderUiEvent_Streaming>(
          this, _$identity);

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is SenderUiEvent_Streaming &&
            (identical(other.framesSent, framesSent) ||
                other.framesSent == framesSent));
  }

  @override
  int get hashCode {
    return Object.hash(runtimeType, framesSent);
  }

  @override
  String toString() {
    return 'SenderUiEvent.streaming(framesSent: $framesSent)';
  }
}

/// @nodoc
abstract mixin class $SenderUiEvent_StreamingCopyWith<$Res>
    implements $SenderUiEventCopyWith<$Res> {
  factory $SenderUiEvent_StreamingCopyWith(SenderUiEvent_Streaming value,
          $Res Function(SenderUiEvent_Streaming) _then) =
      _$SenderUiEvent_StreamingCopyWithImpl;
  @useResult
  $Res call({BigInt framesSent});
}

/// @nodoc
class _$SenderUiEvent_StreamingCopyWithImpl<$Res>
    implements $SenderUiEvent_StreamingCopyWith<$Res> {
  _$SenderUiEvent_StreamingCopyWithImpl(this._self, this._then);

  final SenderUiEvent_Streaming _self;
  final $Res Function(SenderUiEvent_Streaming) _then;

  /// Create a copy of SenderUiEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  $Res call({
    Object? framesSent = null,
  }) {
    return _then(SenderUiEvent_Streaming(
      framesSent: null == framesSent
          ? _self.framesSent
          : framesSent // ignore: cast_nullable_to_non_nullable
              as BigInt,
    ));
  }
}

/// @nodoc

class SenderUiEvent_Finished extends SenderUiEvent {
  const SenderUiEvent_Finished({required this.framesSent}) : super._();

  /// Frames written to the track.
  final BigInt framesSent;

  /// Create a copy of SenderUiEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $SenderUiEvent_FinishedCopyWith<SenderUiEvent_Finished> get copyWith =>
      _$SenderUiEvent_FinishedCopyWithImpl<SenderUiEvent_Finished>(
          this, _$identity);

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is SenderUiEvent_Finished &&
            (identical(other.framesSent, framesSent) ||
                other.framesSent == framesSent));
  }

  @override
  int get hashCode {
    return Object.hash(runtimeType, framesSent);
  }

  @override
  String toString() {
    return 'SenderUiEvent.finished(framesSent: $framesSent)';
  }
}

/// @nodoc
abstract mixin class $SenderUiEvent_FinishedCopyWith<$Res>
    implements $SenderUiEventCopyWith<$Res> {
  factory $SenderUiEvent_FinishedCopyWith(SenderUiEvent_Finished value,
          $Res Function(SenderUiEvent_Finished) _then) =
      _$SenderUiEvent_FinishedCopyWithImpl;
  @useResult
  $Res call({BigInt framesSent});
}

/// @nodoc
class _$SenderUiEvent_FinishedCopyWithImpl<$Res>
    implements $SenderUiEvent_FinishedCopyWith<$Res> {
  _$SenderUiEvent_FinishedCopyWithImpl(this._self, this._then);

  final SenderUiEvent_Finished _self;
  final $Res Function(SenderUiEvent_Finished) _then;

  /// Create a copy of SenderUiEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  $Res call({
    Object? framesSent = null,
  }) {
    return _then(SenderUiEvent_Finished(
      framesSent: null == framesSent
          ? _self.framesSent
          : framesSent // ignore: cast_nullable_to_non_nullable
              as BigInt,
    ));
  }
}

/// @nodoc

class SenderUiEvent_PeerLeft extends SenderUiEvent {
  const SenderUiEvent_PeerLeft() : super._();

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType && other is SenderUiEvent_PeerLeft);
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  String toString() {
    return 'SenderUiEvent.peerLeft()';
  }
}

/// @nodoc

class SenderUiEvent_Error extends SenderUiEvent {
  const SenderUiEvent_Error({required this.message}) : super._();

  /// Technical message in English.
  final String message;

  /// Create a copy of SenderUiEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $SenderUiEvent_ErrorCopyWith<SenderUiEvent_Error> get copyWith =>
      _$SenderUiEvent_ErrorCopyWithImpl<SenderUiEvent_Error>(this, _$identity);

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is SenderUiEvent_Error &&
            (identical(other.message, message) || other.message == message));
  }

  @override
  int get hashCode {
    return Object.hash(runtimeType, message);
  }

  @override
  String toString() {
    return 'SenderUiEvent.error(message: $message)';
  }
}

/// @nodoc
abstract mixin class $SenderUiEvent_ErrorCopyWith<$Res>
    implements $SenderUiEventCopyWith<$Res> {
  factory $SenderUiEvent_ErrorCopyWith(
          SenderUiEvent_Error value, $Res Function(SenderUiEvent_Error) _then) =
      _$SenderUiEvent_ErrorCopyWithImpl;
  @useResult
  $Res call({String message});
}

/// @nodoc
class _$SenderUiEvent_ErrorCopyWithImpl<$Res>
    implements $SenderUiEvent_ErrorCopyWith<$Res> {
  _$SenderUiEvent_ErrorCopyWithImpl(this._self, this._then);

  final SenderUiEvent_Error _self;
  final $Res Function(SenderUiEvent_Error) _then;

  /// Create a copy of SenderUiEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  $Res call({
    Object? message = null,
  }) {
    return _then(SenderUiEvent_Error(
      message: null == message
          ? _self.message
          : message // ignore: cast_nullable_to_non_nullable
              as String,
    ));
  }
}

// dart format on
