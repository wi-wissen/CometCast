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
mixin _$AddressParse {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is AddressParse);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'AddressParse()';
}


}

/// @nodoc
class $AddressParseCopyWith<$Res>  {
$AddressParseCopyWith(AddressParse _, $Res Function(AddressParse) __);
}


/// Adds pattern-matching-related methods to [AddressParse].
extension AddressParsePatterns on AddressParse {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( AddressParse_Ok value)?  ok,TResult Function( AddressParse_Invalid value)?  invalid,required TResult orElse(),}){
final _that = this;
switch (_that) {
case AddressParse_Ok() when ok != null:
return ok(_that);case AddressParse_Invalid() when invalid != null:
return invalid(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( AddressParse_Ok value)  ok,required TResult Function( AddressParse_Invalid value)  invalid,}){
final _that = this;
switch (_that) {
case AddressParse_Ok():
return ok(_that);case AddressParse_Invalid():
return invalid(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( AddressParse_Ok value)?  ok,TResult? Function( AddressParse_Invalid value)?  invalid,}){
final _that = this;
switch (_that) {
case AddressParse_Ok() when ok != null:
return ok(_that);case AddressParse_Invalid() when invalid != null:
return invalid(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String server)?  ok,TResult Function()?  invalid,required TResult orElse(),}) {final _that = this;
switch (_that) {
case AddressParse_Ok() when ok != null:
return ok(_that.server);case AddressParse_Invalid() when invalid != null:
return invalid();case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String server)  ok,required TResult Function()  invalid,}) {final _that = this;
switch (_that) {
case AddressParse_Ok():
return ok(_that.server);case AddressParse_Invalid():
return invalid();}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String server)?  ok,TResult? Function()?  invalid,}) {final _that = this;
switch (_that) {
case AddressParse_Ok() when ok != null:
return ok(_that.server);case AddressParse_Invalid() when invalid != null:
return invalid();case _:
  return null;

}
}

}

/// @nodoc


class AddressParse_Ok extends AddressParse {
  const AddressParse_Ok({required this.server}): super._();
  

/// The server address.
 final  String server;

/// Create a copy of AddressParse
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$AddressParse_OkCopyWith<AddressParse_Ok> get copyWith => _$AddressParse_OkCopyWithImpl<AddressParse_Ok>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is AddressParse_Ok&&(identical(other.server, server) || other.server == server));
}


@override
int get hashCode {
    return Object.hash(runtimeType,server);
}

@override
String toString() {
    return 'AddressParse.ok(server: $server)';
}


}

/// @nodoc
abstract mixin class $AddressParse_OkCopyWith<$Res> implements $AddressParseCopyWith<$Res> {
  factory $AddressParse_OkCopyWith(AddressParse_Ok value, $Res Function(AddressParse_Ok) _then) = _$AddressParse_OkCopyWithImpl;
@useResult
$Res call({
 String server
});




}
/// @nodoc
class _$AddressParse_OkCopyWithImpl<$Res>
    implements $AddressParse_OkCopyWith<$Res> {
  _$AddressParse_OkCopyWithImpl(this._self, this._then);

  final AddressParse_Ok _self;
  final $Res Function(AddressParse_Ok) _then;

/// Create a copy of AddressParse
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? server = null,}) {
  return _then(AddressParse_Ok(
server: null == server ? _self.server : server // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class AddressParse_Invalid extends AddressParse {
  const AddressParse_Invalid(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is AddressParse_Invalid);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'AddressParse.invalid()';
}


}




/// @nodoc
mixin _$AirplayUiEvent {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is AirplayUiEvent);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'AirplayUiEvent()';
}


}

/// @nodoc
class $AirplayUiEventCopyWith<$Res>  {
$AirplayUiEventCopyWith(AirplayUiEvent _, $Res Function(AirplayUiEvent) __);
}


/// Adds pattern-matching-related methods to [AirplayUiEvent].
extension AirplayUiEventPatterns on AirplayUiEvent {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( AirplayUiEvent_Advertising value)?  advertising,TResult Function( AirplayUiEvent_NotAnnouncing value)?  notAnnouncing,TResult Function( AirplayUiEvent_Request value)?  request,TResult Function( AirplayUiEvent_Pin value)?  pin,TResult Function( AirplayUiEvent_PinDone value)?  pinDone,TResult Function( AirplayUiEvent_Mirroring value)?  mirroring,TResult Function( AirplayUiEvent_Failed value)?  failed,TResult Function( AirplayUiEvent_Stopped value)?  stopped,required TResult orElse(),}){
final _that = this;
switch (_that) {
case AirplayUiEvent_Advertising() when advertising != null:
return advertising(_that);case AirplayUiEvent_NotAnnouncing() when notAnnouncing != null:
return notAnnouncing(_that);case AirplayUiEvent_Request() when request != null:
return request(_that);case AirplayUiEvent_Pin() when pin != null:
return pin(_that);case AirplayUiEvent_PinDone() when pinDone != null:
return pinDone(_that);case AirplayUiEvent_Mirroring() when mirroring != null:
return mirroring(_that);case AirplayUiEvent_Failed() when failed != null:
return failed(_that);case AirplayUiEvent_Stopped() when stopped != null:
return stopped(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( AirplayUiEvent_Advertising value)  advertising,required TResult Function( AirplayUiEvent_NotAnnouncing value)  notAnnouncing,required TResult Function( AirplayUiEvent_Request value)  request,required TResult Function( AirplayUiEvent_Pin value)  pin,required TResult Function( AirplayUiEvent_PinDone value)  pinDone,required TResult Function( AirplayUiEvent_Mirroring value)  mirroring,required TResult Function( AirplayUiEvent_Failed value)  failed,required TResult Function( AirplayUiEvent_Stopped value)  stopped,}){
final _that = this;
switch (_that) {
case AirplayUiEvent_Advertising():
return advertising(_that);case AirplayUiEvent_NotAnnouncing():
return notAnnouncing(_that);case AirplayUiEvent_Request():
return request(_that);case AirplayUiEvent_Pin():
return pin(_that);case AirplayUiEvent_PinDone():
return pinDone(_that);case AirplayUiEvent_Mirroring():
return mirroring(_that);case AirplayUiEvent_Failed():
return failed(_that);case AirplayUiEvent_Stopped():
return stopped(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( AirplayUiEvent_Advertising value)?  advertising,TResult? Function( AirplayUiEvent_NotAnnouncing value)?  notAnnouncing,TResult? Function( AirplayUiEvent_Request value)?  request,TResult? Function( AirplayUiEvent_Pin value)?  pin,TResult? Function( AirplayUiEvent_PinDone value)?  pinDone,TResult? Function( AirplayUiEvent_Mirroring value)?  mirroring,TResult? Function( AirplayUiEvent_Failed value)?  failed,TResult? Function( AirplayUiEvent_Stopped value)?  stopped,}){
final _that = this;
switch (_that) {
case AirplayUiEvent_Advertising() when advertising != null:
return advertising(_that);case AirplayUiEvent_NotAnnouncing() when notAnnouncing != null:
return notAnnouncing(_that);case AirplayUiEvent_Request() when request != null:
return request(_that);case AirplayUiEvent_Pin() when pin != null:
return pin(_that);case AirplayUiEvent_PinDone() when pinDone != null:
return pinDone(_that);case AirplayUiEvent_Mirroring() when mirroring != null:
return mirroring(_that);case AirplayUiEvent_Failed() when failed != null:
return failed(_that);case AirplayUiEvent_Stopped() when stopped != null:
return stopped(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  advertising,TResult Function()?  notAnnouncing,TResult Function()?  request,TResult Function( String pin)?  pin,TResult Function()?  pinDone,TResult Function( bool active)?  mirroring,TResult Function( String code)?  failed,TResult Function()?  stopped,required TResult orElse(),}) {final _that = this;
switch (_that) {
case AirplayUiEvent_Advertising() when advertising != null:
return advertising();case AirplayUiEvent_NotAnnouncing() when notAnnouncing != null:
return notAnnouncing();case AirplayUiEvent_Request() when request != null:
return request();case AirplayUiEvent_Pin() when pin != null:
return pin(_that.pin);case AirplayUiEvent_PinDone() when pinDone != null:
return pinDone();case AirplayUiEvent_Mirroring() when mirroring != null:
return mirroring(_that.active);case AirplayUiEvent_Failed() when failed != null:
return failed(_that.code);case AirplayUiEvent_Stopped() when stopped != null:
return stopped();case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  advertising,required TResult Function()  notAnnouncing,required TResult Function()  request,required TResult Function( String pin)  pin,required TResult Function()  pinDone,required TResult Function( bool active)  mirroring,required TResult Function( String code)  failed,required TResult Function()  stopped,}) {final _that = this;
switch (_that) {
case AirplayUiEvent_Advertising():
return advertising();case AirplayUiEvent_NotAnnouncing():
return notAnnouncing();case AirplayUiEvent_Request():
return request();case AirplayUiEvent_Pin():
return pin(_that.pin);case AirplayUiEvent_PinDone():
return pinDone();case AirplayUiEvent_Mirroring():
return mirroring(_that.active);case AirplayUiEvent_Failed():
return failed(_that.code);case AirplayUiEvent_Stopped():
return stopped();}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  advertising,TResult? Function()?  notAnnouncing,TResult? Function()?  request,TResult? Function( String pin)?  pin,TResult? Function()?  pinDone,TResult? Function( bool active)?  mirroring,TResult? Function( String code)?  failed,TResult? Function()?  stopped,}) {final _that = this;
switch (_that) {
case AirplayUiEvent_Advertising() when advertising != null:
return advertising();case AirplayUiEvent_NotAnnouncing() when notAnnouncing != null:
return notAnnouncing();case AirplayUiEvent_Request() when request != null:
return request();case AirplayUiEvent_Pin() when pin != null:
return pin(_that.pin);case AirplayUiEvent_PinDone() when pinDone != null:
return pinDone();case AirplayUiEvent_Mirroring() when mirroring != null:
return mirroring(_that.active);case AirplayUiEvent_Failed() when failed != null:
return failed(_that.code);case AirplayUiEvent_Stopped() when stopped != null:
return stopped();case _:
  return null;

}
}

}

/// @nodoc


class AirplayUiEvent_Advertising extends AirplayUiEvent {
  const AirplayUiEvent_Advertising(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is AirplayUiEvent_Advertising);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'AirplayUiEvent.advertising()';
}


}




/// @nodoc


class AirplayUiEvent_NotAnnouncing extends AirplayUiEvent {
  const AirplayUiEvent_NotAnnouncing(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is AirplayUiEvent_NotAnnouncing);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'AirplayUiEvent.notAnnouncing()';
}


}




/// @nodoc


class AirplayUiEvent_Request extends AirplayUiEvent {
  const AirplayUiEvent_Request(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is AirplayUiEvent_Request);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'AirplayUiEvent.request()';
}


}




/// @nodoc


class AirplayUiEvent_Pin extends AirplayUiEvent {
  const AirplayUiEvent_Pin({required this.pin}): super._();
  

/// Four digits, read from UxPlay.
 final  String pin;

/// Create a copy of AirplayUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$AirplayUiEvent_PinCopyWith<AirplayUiEvent_Pin> get copyWith => _$AirplayUiEvent_PinCopyWithImpl<AirplayUiEvent_Pin>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is AirplayUiEvent_Pin&&(identical(other.pin, pin) || other.pin == pin));
}


@override
int get hashCode {
    return Object.hash(runtimeType,pin);
}

@override
String toString() {
    return 'AirplayUiEvent.pin(pin: $pin)';
}


}

/// @nodoc
abstract mixin class $AirplayUiEvent_PinCopyWith<$Res> implements $AirplayUiEventCopyWith<$Res> {
  factory $AirplayUiEvent_PinCopyWith(AirplayUiEvent_Pin value, $Res Function(AirplayUiEvent_Pin) _then) = _$AirplayUiEvent_PinCopyWithImpl;
@useResult
$Res call({
 String pin
});




}
/// @nodoc
class _$AirplayUiEvent_PinCopyWithImpl<$Res>
    implements $AirplayUiEvent_PinCopyWith<$Res> {
  _$AirplayUiEvent_PinCopyWithImpl(this._self, this._then);

  final AirplayUiEvent_Pin _self;
  final $Res Function(AirplayUiEvent_Pin) _then;

/// Create a copy of AirplayUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? pin = null,}) {
  return _then(AirplayUiEvent_Pin(
pin: null == pin ? _self.pin : pin // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class AirplayUiEvent_PinDone extends AirplayUiEvent {
  const AirplayUiEvent_PinDone(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is AirplayUiEvent_PinDone);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'AirplayUiEvent.pinDone()';
}


}




/// @nodoc


class AirplayUiEvent_Mirroring extends AirplayUiEvent {
  const AirplayUiEvent_Mirroring({required this.active}): super._();
  

/// True while a picture is shown.
 final  bool active;

/// Create a copy of AirplayUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$AirplayUiEvent_MirroringCopyWith<AirplayUiEvent_Mirroring> get copyWith => _$AirplayUiEvent_MirroringCopyWithImpl<AirplayUiEvent_Mirroring>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is AirplayUiEvent_Mirroring&&(identical(other.active, active) || other.active == active));
}


@override
int get hashCode {
    return Object.hash(runtimeType,active);
}

@override
String toString() {
    return 'AirplayUiEvent.mirroring(active: $active)';
}


}

/// @nodoc
abstract mixin class $AirplayUiEvent_MirroringCopyWith<$Res> implements $AirplayUiEventCopyWith<$Res> {
  factory $AirplayUiEvent_MirroringCopyWith(AirplayUiEvent_Mirroring value, $Res Function(AirplayUiEvent_Mirroring) _then) = _$AirplayUiEvent_MirroringCopyWithImpl;
@useResult
$Res call({
 bool active
});




}
/// @nodoc
class _$AirplayUiEvent_MirroringCopyWithImpl<$Res>
    implements $AirplayUiEvent_MirroringCopyWith<$Res> {
  _$AirplayUiEvent_MirroringCopyWithImpl(this._self, this._then);

  final AirplayUiEvent_Mirroring _self;
  final $Res Function(AirplayUiEvent_Mirroring) _then;

/// Create a copy of AirplayUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? active = null,}) {
  return _then(AirplayUiEvent_Mirroring(
active: null == active ? _self.active : active // ignore: cast_nullable_to_non_nullable
as bool,
  ));
}


}

/// @nodoc


class AirplayUiEvent_Failed extends AirplayUiEvent {
  const AirplayUiEvent_Failed({required this.code}): super._();
  

/// The code the app maps to a state line.
 final  String code;

/// Create a copy of AirplayUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$AirplayUiEvent_FailedCopyWith<AirplayUiEvent_Failed> get copyWith => _$AirplayUiEvent_FailedCopyWithImpl<AirplayUiEvent_Failed>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is AirplayUiEvent_Failed&&(identical(other.code, code) || other.code == code));
}


@override
int get hashCode {
    return Object.hash(runtimeType,code);
}

@override
String toString() {
    return 'AirplayUiEvent.failed(code: $code)';
}


}

/// @nodoc
abstract mixin class $AirplayUiEvent_FailedCopyWith<$Res> implements $AirplayUiEventCopyWith<$Res> {
  factory $AirplayUiEvent_FailedCopyWith(AirplayUiEvent_Failed value, $Res Function(AirplayUiEvent_Failed) _then) = _$AirplayUiEvent_FailedCopyWithImpl;
@useResult
$Res call({
 String code
});




}
/// @nodoc
class _$AirplayUiEvent_FailedCopyWithImpl<$Res>
    implements $AirplayUiEvent_FailedCopyWith<$Res> {
  _$AirplayUiEvent_FailedCopyWithImpl(this._self, this._then);

  final AirplayUiEvent_Failed _self;
  final $Res Function(AirplayUiEvent_Failed) _then;

/// Create a copy of AirplayUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? code = null,}) {
  return _then(AirplayUiEvent_Failed(
code: null == code ? _self.code : code // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class AirplayUiEvent_Stopped extends AirplayUiEvent {
  const AirplayUiEvent_Stopped(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is AirplayUiEvent_Stopped);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'AirplayUiEvent.stopped()';
}


}




/// @nodoc
mixin _$CaptureSupport {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is CaptureSupport);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'CaptureSupport()';
}


}

/// @nodoc
class $CaptureSupportCopyWith<$Res>  {
$CaptureSupportCopyWith(CaptureSupport _, $Res Function(CaptureSupport) __);
}


/// Adds pattern-matching-related methods to [CaptureSupport].
extension CaptureSupportPatterns on CaptureSupport {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( CaptureSupport_Supported value)?  supported,TResult Function( CaptureSupport_Wayland value)?  wayland,TResult Function( CaptureSupport_NoDisplay value)?  noDisplay,TResult Function( CaptureSupport_UnsupportedOs value)?  unsupportedOs,TResult Function( CaptureSupport_Failed value)?  failed,required TResult orElse(),}){
final _that = this;
switch (_that) {
case CaptureSupport_Supported() when supported != null:
return supported(_that);case CaptureSupport_Wayland() when wayland != null:
return wayland(_that);case CaptureSupport_NoDisplay() when noDisplay != null:
return noDisplay(_that);case CaptureSupport_UnsupportedOs() when unsupportedOs != null:
return unsupportedOs(_that);case CaptureSupport_Failed() when failed != null:
return failed(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( CaptureSupport_Supported value)  supported,required TResult Function( CaptureSupport_Wayland value)  wayland,required TResult Function( CaptureSupport_NoDisplay value)  noDisplay,required TResult Function( CaptureSupport_UnsupportedOs value)  unsupportedOs,required TResult Function( CaptureSupport_Failed value)  failed,}){
final _that = this;
switch (_that) {
case CaptureSupport_Supported():
return supported(_that);case CaptureSupport_Wayland():
return wayland(_that);case CaptureSupport_NoDisplay():
return noDisplay(_that);case CaptureSupport_UnsupportedOs():
return unsupportedOs(_that);case CaptureSupport_Failed():
return failed(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( CaptureSupport_Supported value)?  supported,TResult? Function( CaptureSupport_Wayland value)?  wayland,TResult? Function( CaptureSupport_NoDisplay value)?  noDisplay,TResult? Function( CaptureSupport_UnsupportedOs value)?  unsupportedOs,TResult? Function( CaptureSupport_Failed value)?  failed,}){
final _that = this;
switch (_that) {
case CaptureSupport_Supported() when supported != null:
return supported(_that);case CaptureSupport_Wayland() when wayland != null:
return wayland(_that);case CaptureSupport_NoDisplay() when noDisplay != null:
return noDisplay(_that);case CaptureSupport_UnsupportedOs() when unsupportedOs != null:
return unsupportedOs(_that);case CaptureSupport_Failed() when failed != null:
return failed(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  supported,TResult Function()?  wayland,TResult Function()?  noDisplay,TResult Function()?  unsupportedOs,TResult Function( String message)?  failed,required TResult orElse(),}) {final _that = this;
switch (_that) {
case CaptureSupport_Supported() when supported != null:
return supported();case CaptureSupport_Wayland() when wayland != null:
return wayland();case CaptureSupport_NoDisplay() when noDisplay != null:
return noDisplay();case CaptureSupport_UnsupportedOs() when unsupportedOs != null:
return unsupportedOs();case CaptureSupport_Failed() when failed != null:
return failed(_that.message);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  supported,required TResult Function()  wayland,required TResult Function()  noDisplay,required TResult Function()  unsupportedOs,required TResult Function( String message)  failed,}) {final _that = this;
switch (_that) {
case CaptureSupport_Supported():
return supported();case CaptureSupport_Wayland():
return wayland();case CaptureSupport_NoDisplay():
return noDisplay();case CaptureSupport_UnsupportedOs():
return unsupportedOs();case CaptureSupport_Failed():
return failed(_that.message);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  supported,TResult? Function()?  wayland,TResult? Function()?  noDisplay,TResult? Function()?  unsupportedOs,TResult? Function( String message)?  failed,}) {final _that = this;
switch (_that) {
case CaptureSupport_Supported() when supported != null:
return supported();case CaptureSupport_Wayland() when wayland != null:
return wayland();case CaptureSupport_NoDisplay() when noDisplay != null:
return noDisplay();case CaptureSupport_UnsupportedOs() when unsupportedOs != null:
return unsupportedOs();case CaptureSupport_Failed() when failed != null:
return failed(_that.message);case _:
  return null;

}
}

}

/// @nodoc


class CaptureSupport_Supported extends CaptureSupport {
  const CaptureSupport_Supported(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is CaptureSupport_Supported);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'CaptureSupport.supported()';
}


}




/// @nodoc


class CaptureSupport_Wayland extends CaptureSupport {
  const CaptureSupport_Wayland(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is CaptureSupport_Wayland);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'CaptureSupport.wayland()';
}


}




/// @nodoc


class CaptureSupport_NoDisplay extends CaptureSupport {
  const CaptureSupport_NoDisplay(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is CaptureSupport_NoDisplay);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'CaptureSupport.noDisplay()';
}


}




/// @nodoc


class CaptureSupport_UnsupportedOs extends CaptureSupport {
  const CaptureSupport_UnsupportedOs(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is CaptureSupport_UnsupportedOs);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'CaptureSupport.unsupportedOs()';
}


}




/// @nodoc


class CaptureSupport_Failed extends CaptureSupport {
  const CaptureSupport_Failed({required this.message}): super._();
  

/// Technical message in English.
 final  String message;

/// Create a copy of CaptureSupport
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CaptureSupport_FailedCopyWith<CaptureSupport_Failed> get copyWith => _$CaptureSupport_FailedCopyWithImpl<CaptureSupport_Failed>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is CaptureSupport_Failed&&(identical(other.message, message) || other.message == message));
}


@override
int get hashCode {
    return Object.hash(runtimeType,message);
}

@override
String toString() {
    return 'CaptureSupport.failed(message: $message)';
}


}

/// @nodoc
abstract mixin class $CaptureSupport_FailedCopyWith<$Res> implements $CaptureSupportCopyWith<$Res> {
  factory $CaptureSupport_FailedCopyWith(CaptureSupport_Failed value, $Res Function(CaptureSupport_Failed) _then) = _$CaptureSupport_FailedCopyWithImpl;
@useResult
$Res call({
 String message
});




}
/// @nodoc
class _$CaptureSupport_FailedCopyWithImpl<$Res>
    implements $CaptureSupport_FailedCopyWith<$Res> {
  _$CaptureSupport_FailedCopyWithImpl(this._self, this._then);

  final CaptureSupport_Failed _self;
  final $Res Function(CaptureSupport_Failed) _then;

/// Create a copy of CaptureSupport
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? message = null,}) {
  return _then(CaptureSupport_Failed(
message: null == message ? _self.message : message // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc
mixin _$ContentFrame {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ContentFrame);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ContentFrame()';
}


}

/// @nodoc
class $ContentFrameCopyWith<$Res>  {
$ContentFrameCopyWith(ContentFrame _, $Res Function(ContentFrame) __);
}


/// Adds pattern-matching-related methods to [ContentFrame].
extension ContentFramePatterns on ContentFrame {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( ContentFrame_Image value)?  image,TResult Function( ContentFrame_Page value)?  page,required TResult orElse(),}){
final _that = this;
switch (_that) {
case ContentFrame_Image() when image != null:
return image(_that);case ContentFrame_Page() when page != null:
return page(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( ContentFrame_Image value)  image,required TResult Function( ContentFrame_Page value)  page,}){
final _that = this;
switch (_that) {
case ContentFrame_Image():
return image(_that);case ContentFrame_Page():
return page(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( ContentFrame_Image value)?  image,TResult? Function( ContentFrame_Page value)?  page,}){
final _that = this;
switch (_that) {
case ContentFrame_Image() when image != null:
return image(_that);case ContentFrame_Page() when page != null:
return page(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String name,  String mime,  Uint8List bytes)?  image,TResult Function( int page,  int width,  int height,  Uint8List rgba)?  page,required TResult orElse(),}) {final _that = this;
switch (_that) {
case ContentFrame_Image() when image != null:
return image(_that.name,_that.mime,_that.bytes);case ContentFrame_Page() when page != null:
return page(_that.page,_that.width,_that.height,_that.rgba);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String name,  String mime,  Uint8List bytes)  image,required TResult Function( int page,  int width,  int height,  Uint8List rgba)  page,}) {final _that = this;
switch (_that) {
case ContentFrame_Image():
return image(_that.name,_that.mime,_that.bytes);case ContentFrame_Page():
return page(_that.page,_that.width,_that.height,_that.rgba);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String name,  String mime,  Uint8List bytes)?  image,TResult? Function( int page,  int width,  int height,  Uint8List rgba)?  page,}) {final _that = this;
switch (_that) {
case ContentFrame_Image() when image != null:
return image(_that.name,_that.mime,_that.bytes);case ContentFrame_Page() when page != null:
return page(_that.page,_that.width,_that.height,_that.rgba);case _:
  return null;

}
}

}

/// @nodoc


class ContentFrame_Image extends ContentFrame {
  const ContentFrame_Image({required this.name, required this.mime, required this.bytes}): super._();
  

/// File name, cleaned.
 final  String name;
/// `image/png`, `image/jpeg`, `image/webp` or `image/gif`.
 final  String mime;
/// The file's bytes (at most 10 MiB).
 final  Uint8List bytes;

/// Create a copy of ContentFrame
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ContentFrame_ImageCopyWith<ContentFrame_Image> get copyWith => _$ContentFrame_ImageCopyWithImpl<ContentFrame_Image>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ContentFrame_Image&&(identical(other.name, name) || other.name == name)&&(identical(other.mime, mime) || other.mime == mime)&&const DeepCollectionEquality().equals(other.bytes, bytes));
}


@override
int get hashCode {
    return Object.hash(runtimeType,name,mime,const DeepCollectionEquality().hash(bytes));
}

@override
String toString() {
    return 'ContentFrame.image(name: $name, mime: $mime, bytes: $bytes)';
}


}

/// @nodoc
abstract mixin class $ContentFrame_ImageCopyWith<$Res> implements $ContentFrameCopyWith<$Res> {
  factory $ContentFrame_ImageCopyWith(ContentFrame_Image value, $Res Function(ContentFrame_Image) _then) = _$ContentFrame_ImageCopyWithImpl;
@useResult
$Res call({
 String name, String mime, Uint8List bytes
});




}
/// @nodoc
class _$ContentFrame_ImageCopyWithImpl<$Res>
    implements $ContentFrame_ImageCopyWith<$Res> {
  _$ContentFrame_ImageCopyWithImpl(this._self, this._then);

  final ContentFrame_Image _self;
  final $Res Function(ContentFrame_Image) _then;

/// Create a copy of ContentFrame
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? name = null,Object? mime = null,Object? bytes = null,}) {
  return _then(ContentFrame_Image(
name: null == name ? _self.name : name // ignore: cast_nullable_to_non_nullable
as String,mime: null == mime ? _self.mime : mime // ignore: cast_nullable_to_non_nullable
as String,bytes: null == bytes ? _self.bytes : bytes // ignore: cast_nullable_to_non_nullable
as Uint8List,
  ));
}


}

/// @nodoc


class ContentFrame_Page extends ContentFrame {
  const ContentFrame_Page({required this.page, required this.width, required this.height, required this.rgba}): super._();
  

/// Page number, 1-based.
 final  int page;
/// Width in pixels.
 final  int width;
/// Height in pixels.
 final  int height;
/// RGBA, rows without padding, opaque white background.
 final  Uint8List rgba;

/// Create a copy of ContentFrame
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ContentFrame_PageCopyWith<ContentFrame_Page> get copyWith => _$ContentFrame_PageCopyWithImpl<ContentFrame_Page>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ContentFrame_Page&&(identical(other.page, page) || other.page == page)&&(identical(other.width, width) || other.width == width)&&(identical(other.height, height) || other.height == height)&&const DeepCollectionEquality().equals(other.rgba, rgba));
}


@override
int get hashCode {
    return Object.hash(runtimeType,page,width,height,const DeepCollectionEquality().hash(rgba));
}

@override
String toString() {
    return 'ContentFrame.page(page: $page, width: $width, height: $height, rgba: $rgba)';
}


}

/// @nodoc
abstract mixin class $ContentFrame_PageCopyWith<$Res> implements $ContentFrameCopyWith<$Res> {
  factory $ContentFrame_PageCopyWith(ContentFrame_Page value, $Res Function(ContentFrame_Page) _then) = _$ContentFrame_PageCopyWithImpl;
@useResult
$Res call({
 int page, int width, int height, Uint8List rgba
});




}
/// @nodoc
class _$ContentFrame_PageCopyWithImpl<$Res>
    implements $ContentFrame_PageCopyWith<$Res> {
  _$ContentFrame_PageCopyWithImpl(this._self, this._then);

  final ContentFrame_Page _self;
  final $Res Function(ContentFrame_Page) _then;

/// Create a copy of ContentFrame
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? page = null,Object? width = null,Object? height = null,Object? rgba = null,}) {
  return _then(ContentFrame_Page(
page: null == page ? _self.page : page // ignore: cast_nullable_to_non_nullable
as int,width: null == width ? _self.width : width // ignore: cast_nullable_to_non_nullable
as int,height: null == height ? _self.height : height // ignore: cast_nullable_to_non_nullable
as int,rgba: null == rgba ? _self.rgba : rgba // ignore: cast_nullable_to_non_nullable
as Uint8List,
  ));
}


}

/// @nodoc
mixin _$ContentSenderUiEvent {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ContentSenderUiEvent);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ContentSenderUiEvent()';
}


}

/// @nodoc
class $ContentSenderUiEventCopyWith<$Res>  {
$ContentSenderUiEventCopyWith(ContentSenderUiEvent _, $Res Function(ContentSenderUiEvent) __);
}


/// Adds pattern-matching-related methods to [ContentSenderUiEvent].
extension ContentSenderUiEventPatterns on ContentSenderUiEvent {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( ContentSenderUiEvent_Prepared value)?  prepared,TResult Function( ContentSenderUiEvent_Converting value)?  converting,TResult Function( ContentSenderUiEvent_WaitingForApproval value)?  waitingForApproval,TResult Function( ContentSenderUiEvent_JoinRefused value)?  joinRefused,TResult Function( ContentSenderUiEvent_RemovedByHost value)?  removedByHost,TResult Function( ContentSenderUiEvent_Joined value)?  joined,TResult Function( ContentSenderUiEvent_ConnectionState value)?  connectionState,TResult Function( ContentSenderUiEvent_Connected value)?  connected,TResult Function( ContentSenderUiEvent_Progress value)?  progress,TResult Function( ContentSenderUiEvent_Sent value)?  sent,TResult Function( ContentSenderUiEvent_Stopped value)?  stopped,TResult Function( ContentSenderUiEvent_PeerLeft value)?  peerLeft,TResult Function( ContentSenderUiEvent_Rejected value)?  rejected,TResult Function( ContentSenderUiEvent_Error value)?  error,required TResult orElse(),}){
final _that = this;
switch (_that) {
case ContentSenderUiEvent_Prepared() when prepared != null:
return prepared(_that);case ContentSenderUiEvent_Converting() when converting != null:
return converting(_that);case ContentSenderUiEvent_WaitingForApproval() when waitingForApproval != null:
return waitingForApproval(_that);case ContentSenderUiEvent_JoinRefused() when joinRefused != null:
return joinRefused(_that);case ContentSenderUiEvent_RemovedByHost() when removedByHost != null:
return removedByHost(_that);case ContentSenderUiEvent_Joined() when joined != null:
return joined(_that);case ContentSenderUiEvent_ConnectionState() when connectionState != null:
return connectionState(_that);case ContentSenderUiEvent_Connected() when connected != null:
return connected(_that);case ContentSenderUiEvent_Progress() when progress != null:
return progress(_that);case ContentSenderUiEvent_Sent() when sent != null:
return sent(_that);case ContentSenderUiEvent_Stopped() when stopped != null:
return stopped(_that);case ContentSenderUiEvent_PeerLeft() when peerLeft != null:
return peerLeft(_that);case ContentSenderUiEvent_Rejected() when rejected != null:
return rejected(_that);case ContentSenderUiEvent_Error() when error != null:
return error(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( ContentSenderUiEvent_Prepared value)  prepared,required TResult Function( ContentSenderUiEvent_Converting value)  converting,required TResult Function( ContentSenderUiEvent_WaitingForApproval value)  waitingForApproval,required TResult Function( ContentSenderUiEvent_JoinRefused value)  joinRefused,required TResult Function( ContentSenderUiEvent_RemovedByHost value)  removedByHost,required TResult Function( ContentSenderUiEvent_Joined value)  joined,required TResult Function( ContentSenderUiEvent_ConnectionState value)  connectionState,required TResult Function( ContentSenderUiEvent_Connected value)  connected,required TResult Function( ContentSenderUiEvent_Progress value)  progress,required TResult Function( ContentSenderUiEvent_Sent value)  sent,required TResult Function( ContentSenderUiEvent_Stopped value)  stopped,required TResult Function( ContentSenderUiEvent_PeerLeft value)  peerLeft,required TResult Function( ContentSenderUiEvent_Rejected value)  rejected,required TResult Function( ContentSenderUiEvent_Error value)  error,}){
final _that = this;
switch (_that) {
case ContentSenderUiEvent_Prepared():
return prepared(_that);case ContentSenderUiEvent_Converting():
return converting(_that);case ContentSenderUiEvent_WaitingForApproval():
return waitingForApproval(_that);case ContentSenderUiEvent_JoinRefused():
return joinRefused(_that);case ContentSenderUiEvent_RemovedByHost():
return removedByHost(_that);case ContentSenderUiEvent_Joined():
return joined(_that);case ContentSenderUiEvent_ConnectionState():
return connectionState(_that);case ContentSenderUiEvent_Connected():
return connected(_that);case ContentSenderUiEvent_Progress():
return progress(_that);case ContentSenderUiEvent_Sent():
return sent(_that);case ContentSenderUiEvent_Stopped():
return stopped(_that);case ContentSenderUiEvent_PeerLeft():
return peerLeft(_that);case ContentSenderUiEvent_Rejected():
return rejected(_that);case ContentSenderUiEvent_Error():
return error(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( ContentSenderUiEvent_Prepared value)?  prepared,TResult? Function( ContentSenderUiEvent_Converting value)?  converting,TResult? Function( ContentSenderUiEvent_WaitingForApproval value)?  waitingForApproval,TResult? Function( ContentSenderUiEvent_JoinRefused value)?  joinRefused,TResult? Function( ContentSenderUiEvent_RemovedByHost value)?  removedByHost,TResult? Function( ContentSenderUiEvent_Joined value)?  joined,TResult? Function( ContentSenderUiEvent_ConnectionState value)?  connectionState,TResult? Function( ContentSenderUiEvent_Connected value)?  connected,TResult? Function( ContentSenderUiEvent_Progress value)?  progress,TResult? Function( ContentSenderUiEvent_Sent value)?  sent,TResult? Function( ContentSenderUiEvent_Stopped value)?  stopped,TResult? Function( ContentSenderUiEvent_PeerLeft value)?  peerLeft,TResult? Function( ContentSenderUiEvent_Rejected value)?  rejected,TResult? Function( ContentSenderUiEvent_Error value)?  error,}){
final _that = this;
switch (_that) {
case ContentSenderUiEvent_Prepared() when prepared != null:
return prepared(_that);case ContentSenderUiEvent_Converting() when converting != null:
return converting(_that);case ContentSenderUiEvent_WaitingForApproval() when waitingForApproval != null:
return waitingForApproval(_that);case ContentSenderUiEvent_JoinRefused() when joinRefused != null:
return joinRefused(_that);case ContentSenderUiEvent_RemovedByHost() when removedByHost != null:
return removedByHost(_that);case ContentSenderUiEvent_Joined() when joined != null:
return joined(_that);case ContentSenderUiEvent_ConnectionState() when connectionState != null:
return connectionState(_that);case ContentSenderUiEvent_Connected() when connected != null:
return connected(_that);case ContentSenderUiEvent_Progress() when progress != null:
return progress(_that);case ContentSenderUiEvent_Sent() when sent != null:
return sent(_that);case ContentSenderUiEvent_Stopped() when stopped != null:
return stopped(_that);case ContentSenderUiEvent_PeerLeft() when peerLeft != null:
return peerLeft(_that);case ContentSenderUiEvent_Rejected() when rejected != null:
return rejected(_that);case ContentSenderUiEvent_Error() when error != null:
return error(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String name,  UiContentKind kind,  BigInt size,  int? pages,  String? host)?  prepared,TResult Function( String name)?  converting,TResult Function( int timeoutSecs,  int check)?  waitingForApproval,TResult Function( String code)?  joinRefused,TResult Function()?  removedByHost,TResult Function()?  joined,TResult Function( UiConnectionState state)?  connectionState,TResult Function()?  connected,TResult Function( BigInt sent,  BigInt total)?  progress,TResult Function()?  sent,TResult Function()?  stopped,TResult Function()?  peerLeft,TResult Function( UiSendRejection reason)?  rejected,TResult Function( String message)?  error,required TResult orElse(),}) {final _that = this;
switch (_that) {
case ContentSenderUiEvent_Prepared() when prepared != null:
return prepared(_that.name,_that.kind,_that.size,_that.pages,_that.host);case ContentSenderUiEvent_Converting() when converting != null:
return converting(_that.name);case ContentSenderUiEvent_WaitingForApproval() when waitingForApproval != null:
return waitingForApproval(_that.timeoutSecs,_that.check);case ContentSenderUiEvent_JoinRefused() when joinRefused != null:
return joinRefused(_that.code);case ContentSenderUiEvent_RemovedByHost() when removedByHost != null:
return removedByHost();case ContentSenderUiEvent_Joined() when joined != null:
return joined();case ContentSenderUiEvent_ConnectionState() when connectionState != null:
return connectionState(_that.state);case ContentSenderUiEvent_Connected() when connected != null:
return connected();case ContentSenderUiEvent_Progress() when progress != null:
return progress(_that.sent,_that.total);case ContentSenderUiEvent_Sent() when sent != null:
return sent();case ContentSenderUiEvent_Stopped() when stopped != null:
return stopped();case ContentSenderUiEvent_PeerLeft() when peerLeft != null:
return peerLeft();case ContentSenderUiEvent_Rejected() when rejected != null:
return rejected(_that.reason);case ContentSenderUiEvent_Error() when error != null:
return error(_that.message);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String name,  UiContentKind kind,  BigInt size,  int? pages,  String? host)  prepared,required TResult Function( String name)  converting,required TResult Function( int timeoutSecs,  int check)  waitingForApproval,required TResult Function( String code)  joinRefused,required TResult Function()  removedByHost,required TResult Function()  joined,required TResult Function( UiConnectionState state)  connectionState,required TResult Function()  connected,required TResult Function( BigInt sent,  BigInt total)  progress,required TResult Function()  sent,required TResult Function()  stopped,required TResult Function()  peerLeft,required TResult Function( UiSendRejection reason)  rejected,required TResult Function( String message)  error,}) {final _that = this;
switch (_that) {
case ContentSenderUiEvent_Prepared():
return prepared(_that.name,_that.kind,_that.size,_that.pages,_that.host);case ContentSenderUiEvent_Converting():
return converting(_that.name);case ContentSenderUiEvent_WaitingForApproval():
return waitingForApproval(_that.timeoutSecs,_that.check);case ContentSenderUiEvent_JoinRefused():
return joinRefused(_that.code);case ContentSenderUiEvent_RemovedByHost():
return removedByHost();case ContentSenderUiEvent_Joined():
return joined();case ContentSenderUiEvent_ConnectionState():
return connectionState(_that.state);case ContentSenderUiEvent_Connected():
return connected();case ContentSenderUiEvent_Progress():
return progress(_that.sent,_that.total);case ContentSenderUiEvent_Sent():
return sent();case ContentSenderUiEvent_Stopped():
return stopped();case ContentSenderUiEvent_PeerLeft():
return peerLeft();case ContentSenderUiEvent_Rejected():
return rejected(_that.reason);case ContentSenderUiEvent_Error():
return error(_that.message);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String name,  UiContentKind kind,  BigInt size,  int? pages,  String? host)?  prepared,TResult? Function( String name)?  converting,TResult? Function( int timeoutSecs,  int check)?  waitingForApproval,TResult? Function( String code)?  joinRefused,TResult? Function()?  removedByHost,TResult? Function()?  joined,TResult? Function( UiConnectionState state)?  connectionState,TResult? Function()?  connected,TResult? Function( BigInt sent,  BigInt total)?  progress,TResult? Function()?  sent,TResult? Function()?  stopped,TResult? Function()?  peerLeft,TResult? Function( UiSendRejection reason)?  rejected,TResult? Function( String message)?  error,}) {final _that = this;
switch (_that) {
case ContentSenderUiEvent_Prepared() when prepared != null:
return prepared(_that.name,_that.kind,_that.size,_that.pages,_that.host);case ContentSenderUiEvent_Converting() when converting != null:
return converting(_that.name);case ContentSenderUiEvent_WaitingForApproval() when waitingForApproval != null:
return waitingForApproval(_that.timeoutSecs,_that.check);case ContentSenderUiEvent_JoinRefused() when joinRefused != null:
return joinRefused(_that.code);case ContentSenderUiEvent_RemovedByHost() when removedByHost != null:
return removedByHost();case ContentSenderUiEvent_Joined() when joined != null:
return joined();case ContentSenderUiEvent_ConnectionState() when connectionState != null:
return connectionState(_that.state);case ContentSenderUiEvent_Connected() when connected != null:
return connected();case ContentSenderUiEvent_Progress() when progress != null:
return progress(_that.sent,_that.total);case ContentSenderUiEvent_Sent() when sent != null:
return sent();case ContentSenderUiEvent_Stopped() when stopped != null:
return stopped();case ContentSenderUiEvent_PeerLeft() when peerLeft != null:
return peerLeft();case ContentSenderUiEvent_Rejected() when rejected != null:
return rejected(_that.reason);case ContentSenderUiEvent_Error() when error != null:
return error(_that.message);case _:
  return null;

}
}

}

/// @nodoc


class ContentSenderUiEvent_Prepared extends ContentSenderUiEvent {
  const ContentSenderUiEvent_Prepared({required this.name, required this.kind, required this.size, this.pages, this.host}): super._();
  

/// File name, or the host of a link.
 final  String name;
/// Image, PDF or link.
 final  UiContentKind kind;
/// Size in bytes (the length of the URL for a link).
 final  BigInt size;
/// Pages of a PDF, if the render worker could tell. `None` = unknown.
 final  int? pages;
/// Host of a link in its punycode form.
 final  String? host;

/// Create a copy of ContentSenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ContentSenderUiEvent_PreparedCopyWith<ContentSenderUiEvent_Prepared> get copyWith => _$ContentSenderUiEvent_PreparedCopyWithImpl<ContentSenderUiEvent_Prepared>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ContentSenderUiEvent_Prepared&&(identical(other.name, name) || other.name == name)&&(identical(other.kind, kind) || other.kind == kind)&&(identical(other.size, size) || other.size == size)&&(identical(other.pages, pages) || other.pages == pages)&&(identical(other.host, host) || other.host == host));
}


@override
int get hashCode {
    return Object.hash(runtimeType,name,kind,size,pages,host);
}

@override
String toString() {
    return 'ContentSenderUiEvent.prepared(name: $name, kind: $kind, size: $size, pages: $pages, host: $host)';
}


}

/// @nodoc
abstract mixin class $ContentSenderUiEvent_PreparedCopyWith<$Res> implements $ContentSenderUiEventCopyWith<$Res> {
  factory $ContentSenderUiEvent_PreparedCopyWith(ContentSenderUiEvent_Prepared value, $Res Function(ContentSenderUiEvent_Prepared) _then) = _$ContentSenderUiEvent_PreparedCopyWithImpl;
@useResult
$Res call({
 String name, UiContentKind kind, BigInt size, int? pages, String? host
});




}
/// @nodoc
class _$ContentSenderUiEvent_PreparedCopyWithImpl<$Res>
    implements $ContentSenderUiEvent_PreparedCopyWith<$Res> {
  _$ContentSenderUiEvent_PreparedCopyWithImpl(this._self, this._then);

  final ContentSenderUiEvent_Prepared _self;
  final $Res Function(ContentSenderUiEvent_Prepared) _then;

/// Create a copy of ContentSenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? name = null,Object? kind = null,Object? size = null,Object? pages = freezed,Object? host = freezed,}) {
  return _then(ContentSenderUiEvent_Prepared(
name: null == name ? _self.name : name // ignore: cast_nullable_to_non_nullable
as String,kind: null == kind ? _self.kind : kind // ignore: cast_nullable_to_non_nullable
as UiContentKind,size: null == size ? _self.size : size // ignore: cast_nullable_to_non_nullable
as BigInt,pages: freezed == pages ? _self.pages : pages // ignore: cast_nullable_to_non_nullable
as int?,host: freezed == host ? _self.host : host // ignore: cast_nullable_to_non_nullable
as String?,
  ));
}


}

/// @nodoc


class ContentSenderUiEvent_Converting extends ContentSenderUiEvent {
  const ContentSenderUiEvent_Converting({required this.name}): super._();
  

/// The file name as picked.
 final  String name;

/// Create a copy of ContentSenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ContentSenderUiEvent_ConvertingCopyWith<ContentSenderUiEvent_Converting> get copyWith => _$ContentSenderUiEvent_ConvertingCopyWithImpl<ContentSenderUiEvent_Converting>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ContentSenderUiEvent_Converting&&(identical(other.name, name) || other.name == name));
}


@override
int get hashCode {
    return Object.hash(runtimeType,name);
}

@override
String toString() {
    return 'ContentSenderUiEvent.converting(name: $name)';
}


}

/// @nodoc
abstract mixin class $ContentSenderUiEvent_ConvertingCopyWith<$Res> implements $ContentSenderUiEventCopyWith<$Res> {
  factory $ContentSenderUiEvent_ConvertingCopyWith(ContentSenderUiEvent_Converting value, $Res Function(ContentSenderUiEvent_Converting) _then) = _$ContentSenderUiEvent_ConvertingCopyWithImpl;
@useResult
$Res call({
 String name
});




}
/// @nodoc
class _$ContentSenderUiEvent_ConvertingCopyWithImpl<$Res>
    implements $ContentSenderUiEvent_ConvertingCopyWith<$Res> {
  _$ContentSenderUiEvent_ConvertingCopyWithImpl(this._self, this._then);

  final ContentSenderUiEvent_Converting _self;
  final $Res Function(ContentSenderUiEvent_Converting) _then;

/// Create a copy of ContentSenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? name = null,}) {
  return _then(ContentSenderUiEvent_Converting(
name: null == name ? _self.name : name // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ContentSenderUiEvent_WaitingForApproval extends ContentSenderUiEvent {
  const ContentSenderUiEvent_WaitingForApproval({required this.timeoutSecs, required this.check}): super._();
  

/// Seconds until the server gives up.
 final  int timeoutSecs;
/// Two-digit check number (the display shows the same number).
 final  int check;

/// Create a copy of ContentSenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ContentSenderUiEvent_WaitingForApprovalCopyWith<ContentSenderUiEvent_WaitingForApproval> get copyWith => _$ContentSenderUiEvent_WaitingForApprovalCopyWithImpl<ContentSenderUiEvent_WaitingForApproval>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ContentSenderUiEvent_WaitingForApproval&&(identical(other.timeoutSecs, timeoutSecs) || other.timeoutSecs == timeoutSecs)&&(identical(other.check, check) || other.check == check));
}


@override
int get hashCode {
    return Object.hash(runtimeType,timeoutSecs,check);
}

@override
String toString() {
    return 'ContentSenderUiEvent.waitingForApproval(timeoutSecs: $timeoutSecs, check: $check)';
}


}

/// @nodoc
abstract mixin class $ContentSenderUiEvent_WaitingForApprovalCopyWith<$Res> implements $ContentSenderUiEventCopyWith<$Res> {
  factory $ContentSenderUiEvent_WaitingForApprovalCopyWith(ContentSenderUiEvent_WaitingForApproval value, $Res Function(ContentSenderUiEvent_WaitingForApproval) _then) = _$ContentSenderUiEvent_WaitingForApprovalCopyWithImpl;
@useResult
$Res call({
 int timeoutSecs, int check
});




}
/// @nodoc
class _$ContentSenderUiEvent_WaitingForApprovalCopyWithImpl<$Res>
    implements $ContentSenderUiEvent_WaitingForApprovalCopyWith<$Res> {
  _$ContentSenderUiEvent_WaitingForApprovalCopyWithImpl(this._self, this._then);

  final ContentSenderUiEvent_WaitingForApproval _self;
  final $Res Function(ContentSenderUiEvent_WaitingForApproval) _then;

/// Create a copy of ContentSenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? timeoutSecs = null,Object? check = null,}) {
  return _then(ContentSenderUiEvent_WaitingForApproval(
timeoutSecs: null == timeoutSecs ? _self.timeoutSecs : timeoutSecs // ignore: cast_nullable_to_non_nullable
as int,check: null == check ? _self.check : check // ignore: cast_nullable_to_non_nullable
as int,
  ));
}


}

/// @nodoc


class ContentSenderUiEvent_JoinRefused extends ContentSenderUiEvent {
  const ContentSenderUiEvent_JoinRefused({required this.code}): super._();
  

/// Server code.
 final  String code;

/// Create a copy of ContentSenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ContentSenderUiEvent_JoinRefusedCopyWith<ContentSenderUiEvent_JoinRefused> get copyWith => _$ContentSenderUiEvent_JoinRefusedCopyWithImpl<ContentSenderUiEvent_JoinRefused>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ContentSenderUiEvent_JoinRefused&&(identical(other.code, code) || other.code == code));
}


@override
int get hashCode {
    return Object.hash(runtimeType,code);
}

@override
String toString() {
    return 'ContentSenderUiEvent.joinRefused(code: $code)';
}


}

/// @nodoc
abstract mixin class $ContentSenderUiEvent_JoinRefusedCopyWith<$Res> implements $ContentSenderUiEventCopyWith<$Res> {
  factory $ContentSenderUiEvent_JoinRefusedCopyWith(ContentSenderUiEvent_JoinRefused value, $Res Function(ContentSenderUiEvent_JoinRefused) _then) = _$ContentSenderUiEvent_JoinRefusedCopyWithImpl;
@useResult
$Res call({
 String code
});




}
/// @nodoc
class _$ContentSenderUiEvent_JoinRefusedCopyWithImpl<$Res>
    implements $ContentSenderUiEvent_JoinRefusedCopyWith<$Res> {
  _$ContentSenderUiEvent_JoinRefusedCopyWithImpl(this._self, this._then);

  final ContentSenderUiEvent_JoinRefused _self;
  final $Res Function(ContentSenderUiEvent_JoinRefused) _then;

/// Create a copy of ContentSenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? code = null,}) {
  return _then(ContentSenderUiEvent_JoinRefused(
code: null == code ? _self.code : code // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ContentSenderUiEvent_RemovedByHost extends ContentSenderUiEvent {
  const ContentSenderUiEvent_RemovedByHost(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ContentSenderUiEvent_RemovedByHost);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ContentSenderUiEvent.removedByHost()';
}


}




/// @nodoc


class ContentSenderUiEvent_Joined extends ContentSenderUiEvent {
  const ContentSenderUiEvent_Joined(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ContentSenderUiEvent_Joined);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ContentSenderUiEvent.joined()';
}


}




/// @nodoc


class ContentSenderUiEvent_ConnectionState extends ContentSenderUiEvent {
  const ContentSenderUiEvent_ConnectionState({required this.state}): super._();
  

/// New state.
 final  UiConnectionState state;

/// Create a copy of ContentSenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ContentSenderUiEvent_ConnectionStateCopyWith<ContentSenderUiEvent_ConnectionState> get copyWith => _$ContentSenderUiEvent_ConnectionStateCopyWithImpl<ContentSenderUiEvent_ConnectionState>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ContentSenderUiEvent_ConnectionState&&(identical(other.state, state) || other.state == state));
}


@override
int get hashCode {
    return Object.hash(runtimeType,state);
}

@override
String toString() {
    return 'ContentSenderUiEvent.connectionState(state: $state)';
}


}

/// @nodoc
abstract mixin class $ContentSenderUiEvent_ConnectionStateCopyWith<$Res> implements $ContentSenderUiEventCopyWith<$Res> {
  factory $ContentSenderUiEvent_ConnectionStateCopyWith(ContentSenderUiEvent_ConnectionState value, $Res Function(ContentSenderUiEvent_ConnectionState) _then) = _$ContentSenderUiEvent_ConnectionStateCopyWithImpl;
@useResult
$Res call({
 UiConnectionState state
});




}
/// @nodoc
class _$ContentSenderUiEvent_ConnectionStateCopyWithImpl<$Res>
    implements $ContentSenderUiEvent_ConnectionStateCopyWith<$Res> {
  _$ContentSenderUiEvent_ConnectionStateCopyWithImpl(this._self, this._then);

  final ContentSenderUiEvent_ConnectionState _self;
  final $Res Function(ContentSenderUiEvent_ConnectionState) _then;

/// Create a copy of ContentSenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? state = null,}) {
  return _then(ContentSenderUiEvent_ConnectionState(
state: null == state ? _self.state : state // ignore: cast_nullable_to_non_nullable
as UiConnectionState,
  ));
}


}

/// @nodoc


class ContentSenderUiEvent_Connected extends ContentSenderUiEvent {
  const ContentSenderUiEvent_Connected(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ContentSenderUiEvent_Connected);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ContentSenderUiEvent.connected()';
}


}




/// @nodoc


class ContentSenderUiEvent_Progress extends ContentSenderUiEvent {
  const ContentSenderUiEvent_Progress({required this.sent, required this.total}): super._();
  

/// Bytes so far.
 final  BigInt sent;
/// Bytes in total.
 final  BigInt total;

/// Create a copy of ContentSenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ContentSenderUiEvent_ProgressCopyWith<ContentSenderUiEvent_Progress> get copyWith => _$ContentSenderUiEvent_ProgressCopyWithImpl<ContentSenderUiEvent_Progress>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ContentSenderUiEvent_Progress&&(identical(other.sent, sent) || other.sent == sent)&&(identical(other.total, total) || other.total == total));
}


@override
int get hashCode {
    return Object.hash(runtimeType,sent,total);
}

@override
String toString() {
    return 'ContentSenderUiEvent.progress(sent: $sent, total: $total)';
}


}

/// @nodoc
abstract mixin class $ContentSenderUiEvent_ProgressCopyWith<$Res> implements $ContentSenderUiEventCopyWith<$Res> {
  factory $ContentSenderUiEvent_ProgressCopyWith(ContentSenderUiEvent_Progress value, $Res Function(ContentSenderUiEvent_Progress) _then) = _$ContentSenderUiEvent_ProgressCopyWithImpl;
@useResult
$Res call({
 BigInt sent, BigInt total
});




}
/// @nodoc
class _$ContentSenderUiEvent_ProgressCopyWithImpl<$Res>
    implements $ContentSenderUiEvent_ProgressCopyWith<$Res> {
  _$ContentSenderUiEvent_ProgressCopyWithImpl(this._self, this._then);

  final ContentSenderUiEvent_Progress _self;
  final $Res Function(ContentSenderUiEvent_Progress) _then;

/// Create a copy of ContentSenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? sent = null,Object? total = null,}) {
  return _then(ContentSenderUiEvent_Progress(
sent: null == sent ? _self.sent : sent // ignore: cast_nullable_to_non_nullable
as BigInt,total: null == total ? _self.total : total // ignore: cast_nullable_to_non_nullable
as BigInt,
  ));
}


}

/// @nodoc


class ContentSenderUiEvent_Sent extends ContentSenderUiEvent {
  const ContentSenderUiEvent_Sent(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ContentSenderUiEvent_Sent);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ContentSenderUiEvent.sent()';
}


}




/// @nodoc


class ContentSenderUiEvent_Stopped extends ContentSenderUiEvent {
  const ContentSenderUiEvent_Stopped(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ContentSenderUiEvent_Stopped);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ContentSenderUiEvent.stopped()';
}


}




/// @nodoc


class ContentSenderUiEvent_PeerLeft extends ContentSenderUiEvent {
  const ContentSenderUiEvent_PeerLeft(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ContentSenderUiEvent_PeerLeft);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ContentSenderUiEvent.peerLeft()';
}


}




/// @nodoc


class ContentSenderUiEvent_Rejected extends ContentSenderUiEvent {
  const ContentSenderUiEvent_Rejected({required this.reason}): super._();
  

/// Why.
 final  UiSendRejection reason;

/// Create a copy of ContentSenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ContentSenderUiEvent_RejectedCopyWith<ContentSenderUiEvent_Rejected> get copyWith => _$ContentSenderUiEvent_RejectedCopyWithImpl<ContentSenderUiEvent_Rejected>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ContentSenderUiEvent_Rejected&&(identical(other.reason, reason) || other.reason == reason));
}


@override
int get hashCode {
    return Object.hash(runtimeType,reason);
}

@override
String toString() {
    return 'ContentSenderUiEvent.rejected(reason: $reason)';
}


}

/// @nodoc
abstract mixin class $ContentSenderUiEvent_RejectedCopyWith<$Res> implements $ContentSenderUiEventCopyWith<$Res> {
  factory $ContentSenderUiEvent_RejectedCopyWith(ContentSenderUiEvent_Rejected value, $Res Function(ContentSenderUiEvent_Rejected) _then) = _$ContentSenderUiEvent_RejectedCopyWithImpl;
@useResult
$Res call({
 UiSendRejection reason
});




}
/// @nodoc
class _$ContentSenderUiEvent_RejectedCopyWithImpl<$Res>
    implements $ContentSenderUiEvent_RejectedCopyWith<$Res> {
  _$ContentSenderUiEvent_RejectedCopyWithImpl(this._self, this._then);

  final ContentSenderUiEvent_Rejected _self;
  final $Res Function(ContentSenderUiEvent_Rejected) _then;

/// Create a copy of ContentSenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? reason = null,}) {
  return _then(ContentSenderUiEvent_Rejected(
reason: null == reason ? _self.reason : reason // ignore: cast_nullable_to_non_nullable
as UiSendRejection,
  ));
}


}

/// @nodoc


class ContentSenderUiEvent_Error extends ContentSenderUiEvent {
  const ContentSenderUiEvent_Error({required this.message}): super._();
  

/// Technical message in English.
 final  String message;

/// Create a copy of ContentSenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ContentSenderUiEvent_ErrorCopyWith<ContentSenderUiEvent_Error> get copyWith => _$ContentSenderUiEvent_ErrorCopyWithImpl<ContentSenderUiEvent_Error>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ContentSenderUiEvent_Error&&(identical(other.message, message) || other.message == message));
}


@override
int get hashCode {
    return Object.hash(runtimeType,message);
}

@override
String toString() {
    return 'ContentSenderUiEvent.error(message: $message)';
}


}

/// @nodoc
abstract mixin class $ContentSenderUiEvent_ErrorCopyWith<$Res> implements $ContentSenderUiEventCopyWith<$Res> {
  factory $ContentSenderUiEvent_ErrorCopyWith(ContentSenderUiEvent_Error value, $Res Function(ContentSenderUiEvent_Error) _then) = _$ContentSenderUiEvent_ErrorCopyWithImpl;
@useResult
$Res call({
 String message
});




}
/// @nodoc
class _$ContentSenderUiEvent_ErrorCopyWithImpl<$Res>
    implements $ContentSenderUiEvent_ErrorCopyWith<$Res> {
  _$ContentSenderUiEvent_ErrorCopyWithImpl(this._self, this._then);

  final ContentSenderUiEvent_Error _self;
  final $Res Function(ContentSenderUiEvent_Error) _then;

/// Create a copy of ContentSenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? message = null,}) {
  return _then(ContentSenderUiEvent_Error(
message: null == message ? _self.message : message // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc
mixin _$ContentSource {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ContentSource);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ContentSource()';
}


}

/// @nodoc
class $ContentSourceCopyWith<$Res>  {
$ContentSourceCopyWith(ContentSource _, $Res Function(ContentSource) __);
}


/// Adds pattern-matching-related methods to [ContentSource].
extension ContentSourcePatterns on ContentSource {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( ContentSource_File value)?  file,TResult Function( ContentSource_Link value)?  link,required TResult orElse(),}){
final _that = this;
switch (_that) {
case ContentSource_File() when file != null:
return file(_that);case ContentSource_Link() when link != null:
return link(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( ContentSource_File value)  file,required TResult Function( ContentSource_Link value)  link,}){
final _that = this;
switch (_that) {
case ContentSource_File():
return file(_that);case ContentSource_Link():
return link(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( ContentSource_File value)?  file,TResult? Function( ContentSource_Link value)?  link,}){
final _that = this;
switch (_that) {
case ContentSource_File() when file != null:
return file(_that);case ContentSource_Link() when link != null:
return link(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String path)?  file,TResult Function( String url)?  link,required TResult orElse(),}) {final _that = this;
switch (_that) {
case ContentSource_File() when file != null:
return file(_that.path);case ContentSource_Link() when link != null:
return link(_that.url);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String path)  file,required TResult Function( String url)  link,}) {final _that = this;
switch (_that) {
case ContentSource_File():
return file(_that.path);case ContentSource_Link():
return link(_that.url);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String path)?  file,TResult? Function( String url)?  link,}) {final _that = this;
switch (_that) {
case ContentSource_File() when file != null:
return file(_that.path);case ContentSource_Link() when link != null:
return link(_that.url);case _:
  return null;

}
}

}

/// @nodoc


class ContentSource_File extends ContentSource {
  const ContentSource_File({required this.path}): super._();
  

/// Path of the file.
 final  String path;

/// Create a copy of ContentSource
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ContentSource_FileCopyWith<ContentSource_File> get copyWith => _$ContentSource_FileCopyWithImpl<ContentSource_File>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ContentSource_File&&(identical(other.path, path) || other.path == path));
}


@override
int get hashCode {
    return Object.hash(runtimeType,path);
}

@override
String toString() {
    return 'ContentSource.file(path: $path)';
}


}

/// @nodoc
abstract mixin class $ContentSource_FileCopyWith<$Res> implements $ContentSourceCopyWith<$Res> {
  factory $ContentSource_FileCopyWith(ContentSource_File value, $Res Function(ContentSource_File) _then) = _$ContentSource_FileCopyWithImpl;
@useResult
$Res call({
 String path
});




}
/// @nodoc
class _$ContentSource_FileCopyWithImpl<$Res>
    implements $ContentSource_FileCopyWith<$Res> {
  _$ContentSource_FileCopyWithImpl(this._self, this._then);

  final ContentSource_File _self;
  final $Res Function(ContentSource_File) _then;

/// Create a copy of ContentSource
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? path = null,}) {
  return _then(ContentSource_File(
path: null == path ? _self.path : path // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ContentSource_Link extends ContentSource {
  const ContentSource_Link({required this.url}): super._();
  

/// The link as typed.
 final  String url;

/// Create a copy of ContentSource
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ContentSource_LinkCopyWith<ContentSource_Link> get copyWith => _$ContentSource_LinkCopyWithImpl<ContentSource_Link>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ContentSource_Link&&(identical(other.url, url) || other.url == url));
}


@override
int get hashCode {
    return Object.hash(runtimeType,url);
}

@override
String toString() {
    return 'ContentSource.link(url: $url)';
}


}

/// @nodoc
abstract mixin class $ContentSource_LinkCopyWith<$Res> implements $ContentSourceCopyWith<$Res> {
  factory $ContentSource_LinkCopyWith(ContentSource_Link value, $Res Function(ContentSource_Link) _then) = _$ContentSource_LinkCopyWithImpl;
@useResult
$Res call({
 String url
});




}
/// @nodoc
class _$ContentSource_LinkCopyWithImpl<$Res>
    implements $ContentSource_LinkCopyWith<$Res> {
  _$ContentSource_LinkCopyWithImpl(this._self, this._then);

  final ContentSource_Link _self;
  final $Res Function(ContentSource_Link) _then;

/// Create a copy of ContentSource
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? url = null,}) {
  return _then(ContentSource_Link(
url: null == url ? _self.url : url // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc
mixin _$LinkCheck {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is LinkCheck);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'LinkCheck()';
}


}

/// @nodoc
class $LinkCheckCopyWith<$Res>  {
$LinkCheckCopyWith(LinkCheck _, $Res Function(LinkCheck) __);
}


/// Adds pattern-matching-related methods to [LinkCheck].
extension LinkCheckPatterns on LinkCheck {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( LinkCheck_Valid value)?  valid,TResult Function( LinkCheck_Invalid value)?  invalid,required TResult orElse(),}){
final _that = this;
switch (_that) {
case LinkCheck_Valid() when valid != null:
return valid(_that);case LinkCheck_Invalid() when invalid != null:
return invalid(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( LinkCheck_Valid value)  valid,required TResult Function( LinkCheck_Invalid value)  invalid,}){
final _that = this;
switch (_that) {
case LinkCheck_Valid():
return valid(_that);case LinkCheck_Invalid():
return invalid(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( LinkCheck_Valid value)?  valid,TResult? Function( LinkCheck_Invalid value)?  invalid,}){
final _that = this;
switch (_that) {
case LinkCheck_Valid() when valid != null:
return valid(_that);case LinkCheck_Invalid() when invalid != null:
return invalid(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String url,  String host)?  valid,TResult Function()?  invalid,required TResult orElse(),}) {final _that = this;
switch (_that) {
case LinkCheck_Valid() when valid != null:
return valid(_that.url,_that.host);case LinkCheck_Invalid() when invalid != null:
return invalid();case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String url,  String host)  valid,required TResult Function()  invalid,}) {final _that = this;
switch (_that) {
case LinkCheck_Valid():
return valid(_that.url,_that.host);case LinkCheck_Invalid():
return invalid();}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String url,  String host)?  valid,TResult? Function()?  invalid,}) {final _that = this;
switch (_that) {
case LinkCheck_Valid() when valid != null:
return valid(_that.url,_that.host);case LinkCheck_Invalid() when invalid != null:
return invalid();case _:
  return null;

}
}

}

/// @nodoc


class LinkCheck_Valid extends LinkCheck {
  const LinkCheck_Valid({required this.url, required this.host}): super._();
  

/// The normalised URL (ASCII), the one to open.
 final  String url;
/// Host in its punycode form.
 final  String host;

/// Create a copy of LinkCheck
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LinkCheck_ValidCopyWith<LinkCheck_Valid> get copyWith => _$LinkCheck_ValidCopyWithImpl<LinkCheck_Valid>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is LinkCheck_Valid&&(identical(other.url, url) || other.url == url)&&(identical(other.host, host) || other.host == host));
}


@override
int get hashCode {
    return Object.hash(runtimeType,url,host);
}

@override
String toString() {
    return 'LinkCheck.valid(url: $url, host: $host)';
}


}

/// @nodoc
abstract mixin class $LinkCheck_ValidCopyWith<$Res> implements $LinkCheckCopyWith<$Res> {
  factory $LinkCheck_ValidCopyWith(LinkCheck_Valid value, $Res Function(LinkCheck_Valid) _then) = _$LinkCheck_ValidCopyWithImpl;
@useResult
$Res call({
 String url, String host
});




}
/// @nodoc
class _$LinkCheck_ValidCopyWithImpl<$Res>
    implements $LinkCheck_ValidCopyWith<$Res> {
  _$LinkCheck_ValidCopyWithImpl(this._self, this._then);

  final LinkCheck_Valid _self;
  final $Res Function(LinkCheck_Valid) _then;

/// Create a copy of LinkCheck
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? url = null,Object? host = null,}) {
  return _then(LinkCheck_Valid(
url: null == url ? _self.url : url // ignore: cast_nullable_to_non_nullable
as String,host: null == host ? _self.host : host // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class LinkCheck_Invalid extends LinkCheck {
  const LinkCheck_Invalid(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is LinkCheck_Invalid);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'LinkCheck.invalid()';
}


}




/// @nodoc
mixin _$NativeAnnounceEvent {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NativeAnnounceEvent);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'NativeAnnounceEvent()';
}


}

/// @nodoc
class $NativeAnnounceEventCopyWith<$Res>  {
$NativeAnnounceEventCopyWith(NativeAnnounceEvent _, $Res Function(NativeAnnounceEvent) __);
}


/// Adds pattern-matching-related methods to [NativeAnnounceEvent].
extension NativeAnnounceEventPatterns on NativeAnnounceEvent {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( NativeAnnounceEvent_Publish value)?  publish,TResult Function( NativeAnnounceEvent_Withdraw value)?  withdraw,required TResult orElse(),}){
final _that = this;
switch (_that) {
case NativeAnnounceEvent_Publish() when publish != null:
return publish(_that);case NativeAnnounceEvent_Withdraw() when withdraw != null:
return withdraw(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( NativeAnnounceEvent_Publish value)  publish,required TResult Function( NativeAnnounceEvent_Withdraw value)  withdraw,}){
final _that = this;
switch (_that) {
case NativeAnnounceEvent_Publish():
return publish(_that);case NativeAnnounceEvent_Withdraw():
return withdraw(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( NativeAnnounceEvent_Publish value)?  publish,TResult? Function( NativeAnnounceEvent_Withdraw value)?  withdraw,}){
final _that = this;
switch (_that) {
case NativeAnnounceEvent_Publish() when publish != null:
return publish(_that);case NativeAnnounceEvent_Withdraw() when withdraw != null:
return withdraw(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String instance,  List<(String, String,)> txt)?  publish,TResult Function()?  withdraw,required TResult orElse(),}) {final _that = this;
switch (_that) {
case NativeAnnounceEvent_Publish() when publish != null:
return publish(_that.instance,_that.txt);case NativeAnnounceEvent_Withdraw() when withdraw != null:
return withdraw();case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String instance,  List<(String, String,)> txt)  publish,required TResult Function()  withdraw,}) {final _that = this;
switch (_that) {
case NativeAnnounceEvent_Publish():
return publish(_that.instance,_that.txt);case NativeAnnounceEvent_Withdraw():
return withdraw();}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String instance,  List<(String, String,)> txt)?  publish,TResult? Function()?  withdraw,}) {final _that = this;
switch (_that) {
case NativeAnnounceEvent_Publish() when publish != null:
return publish(_that.instance,_that.txt);case NativeAnnounceEvent_Withdraw() when withdraw != null:
return withdraw();case _:
  return null;

}
}

}

/// @nodoc


class NativeAnnounceEvent_Publish extends NativeAnnounceEvent {
  const NativeAnnounceEvent_Publish({required this.instance, required  List<(String, String,)> txt}): _txt = txt,super._();
  

/// The DNS-SD instance name.
 final  String instance;
/// The TXT entries.
 final  List<(String, String,)> _txt;
/// The TXT entries.
 List<(String, String,)> get txt {
  if (_txt is EqualUnmodifiableListView) return _txt;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_txt);
}


/// Create a copy of NativeAnnounceEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$NativeAnnounceEvent_PublishCopyWith<NativeAnnounceEvent_Publish> get copyWith => _$NativeAnnounceEvent_PublishCopyWithImpl<NativeAnnounceEvent_Publish>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NativeAnnounceEvent_Publish&&(identical(other.instance, instance) || other.instance == instance)&&const DeepCollectionEquality().equals(other.txt, _txt));
}


@override
int get hashCode {
    return Object.hash(runtimeType,instance,const DeepCollectionEquality().hash(_txt));
}

@override
String toString() {
    return 'NativeAnnounceEvent.publish(instance: $instance, txt: $txt)';
}


}

/// @nodoc
abstract mixin class $NativeAnnounceEvent_PublishCopyWith<$Res> implements $NativeAnnounceEventCopyWith<$Res> {
  factory $NativeAnnounceEvent_PublishCopyWith(NativeAnnounceEvent_Publish value, $Res Function(NativeAnnounceEvent_Publish) _then) = _$NativeAnnounceEvent_PublishCopyWithImpl;
@useResult
$Res call({
 String instance, List<(String, String,)> txt
});




}
/// @nodoc
class _$NativeAnnounceEvent_PublishCopyWithImpl<$Res>
    implements $NativeAnnounceEvent_PublishCopyWith<$Res> {
  _$NativeAnnounceEvent_PublishCopyWithImpl(this._self, this._then);

  final NativeAnnounceEvent_Publish _self;
  final $Res Function(NativeAnnounceEvent_Publish) _then;

/// Create a copy of NativeAnnounceEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? instance = null,Object? txt = null,}) {
  return _then(NativeAnnounceEvent_Publish(
instance: null == instance ? _self.instance : instance // ignore: cast_nullable_to_non_nullable
as String,txt: null == txt ? _self._txt : txt // ignore: cast_nullable_to_non_nullable
as List<(String, String,)>,
  ));
}


}

/// @nodoc


class NativeAnnounceEvent_Withdraw extends NativeAnnounceEvent {
  const NativeAnnounceEvent_Withdraw(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NativeAnnounceEvent_Withdraw);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'NativeAnnounceEvent.withdraw()';
}


}




/// @nodoc
mixin _$NearbyReceiverEvent {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NearbyReceiverEvent);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'NearbyReceiverEvent()';
}


}

/// @nodoc
class $NearbyReceiverEventCopyWith<$Res>  {
$NearbyReceiverEventCopyWith(NearbyReceiverEvent _, $Res Function(NearbyReceiverEvent) __);
}


/// Adds pattern-matching-related methods to [NearbyReceiverEvent].
extension NearbyReceiverEventPatterns on NearbyReceiverEvent {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( NearbyReceiverEvent_Found value)?  found,TResult Function( NearbyReceiverEvent_Lost value)?  lost,TResult Function( NearbyReceiverEvent_Unavailable value)?  unavailable,required TResult orElse(),}){
final _that = this;
switch (_that) {
case NearbyReceiverEvent_Found() when found != null:
return found(_that);case NearbyReceiverEvent_Lost() when lost != null:
return lost(_that);case NearbyReceiverEvent_Unavailable() when unavailable != null:
return unavailable(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( NearbyReceiverEvent_Found value)  found,required TResult Function( NearbyReceiverEvent_Lost value)  lost,required TResult Function( NearbyReceiverEvent_Unavailable value)  unavailable,}){
final _that = this;
switch (_that) {
case NearbyReceiverEvent_Found():
return found(_that);case NearbyReceiverEvent_Lost():
return lost(_that);case NearbyReceiverEvent_Unavailable():
return unavailable(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( NearbyReceiverEvent_Found value)?  found,TResult? Function( NearbyReceiverEvent_Lost value)?  lost,TResult? Function( NearbyReceiverEvent_Unavailable value)?  unavailable,}){
final _that = this;
switch (_that) {
case NearbyReceiverEvent_Found() when found != null:
return found(_that);case NearbyReceiverEvent_Lost() when lost != null:
return lost(_that);case NearbyReceiverEvent_Unavailable() when unavailable != null:
return unavailable(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String id,  String name,  String? server)?  found,TResult Function( String id)?  lost,TResult Function()?  unavailable,required TResult orElse(),}) {final _that = this;
switch (_that) {
case NearbyReceiverEvent_Found() when found != null:
return found(_that.id,_that.name,_that.server);case NearbyReceiverEvent_Lost() when lost != null:
return lost(_that.id);case NearbyReceiverEvent_Unavailable() when unavailable != null:
return unavailable();case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String id,  String name,  String? server)  found,required TResult Function( String id)  lost,required TResult Function()  unavailable,}) {final _that = this;
switch (_that) {
case NearbyReceiverEvent_Found():
return found(_that.id,_that.name,_that.server);case NearbyReceiverEvent_Lost():
return lost(_that.id);case NearbyReceiverEvent_Unavailable():
return unavailable();}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String id,  String name,  String? server)?  found,TResult? Function( String id)?  lost,TResult? Function()?  unavailable,}) {final _that = this;
switch (_that) {
case NearbyReceiverEvent_Found() when found != null:
return found(_that.id,_that.name,_that.server);case NearbyReceiverEvent_Lost() when lost != null:
return lost(_that.id);case NearbyReceiverEvent_Unavailable() when unavailable != null:
return unavailable();case _:
  return null;

}
}

}

/// @nodoc


class NearbyReceiverEvent_Found extends NearbyReceiverEvent {
  const NearbyReceiverEvent_Found({required this.id, required this.name, this.server}): super._();
  

/// The DNS-SD instance name; identifies the receiver in `Lost`.
 final  String id;
/// Friendly name of the display device.
 final  String name;
/// The server it prefers (`[scheme://]host:port`), if it announced a valid one. It is
/// not trusted: the person decides. The room code is never part of an announcement.
 final  String? server;

/// Create a copy of NearbyReceiverEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$NearbyReceiverEvent_FoundCopyWith<NearbyReceiverEvent_Found> get copyWith => _$NearbyReceiverEvent_FoundCopyWithImpl<NearbyReceiverEvent_Found>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NearbyReceiverEvent_Found&&(identical(other.id, id) || other.id == id)&&(identical(other.name, name) || other.name == name)&&(identical(other.server, server) || other.server == server));
}


@override
int get hashCode {
    return Object.hash(runtimeType,id,name,server);
}

@override
String toString() {
    return 'NearbyReceiverEvent.found(id: $id, name: $name, server: $server)';
}


}

/// @nodoc
abstract mixin class $NearbyReceiverEvent_FoundCopyWith<$Res> implements $NearbyReceiverEventCopyWith<$Res> {
  factory $NearbyReceiverEvent_FoundCopyWith(NearbyReceiverEvent_Found value, $Res Function(NearbyReceiverEvent_Found) _then) = _$NearbyReceiverEvent_FoundCopyWithImpl;
@useResult
$Res call({
 String id, String name, String? server
});




}
/// @nodoc
class _$NearbyReceiverEvent_FoundCopyWithImpl<$Res>
    implements $NearbyReceiverEvent_FoundCopyWith<$Res> {
  _$NearbyReceiverEvent_FoundCopyWithImpl(this._self, this._then);

  final NearbyReceiverEvent_Found _self;
  final $Res Function(NearbyReceiverEvent_Found) _then;

/// Create a copy of NearbyReceiverEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? id = null,Object? name = null,Object? server = freezed,}) {
  return _then(NearbyReceiverEvent_Found(
id: null == id ? _self.id : id // ignore: cast_nullable_to_non_nullable
as String,name: null == name ? _self.name : name // ignore: cast_nullable_to_non_nullable
as String,server: freezed == server ? _self.server : server // ignore: cast_nullable_to_non_nullable
as String?,
  ));
}


}

/// @nodoc


class NearbyReceiverEvent_Lost extends NearbyReceiverEvent {
  const NearbyReceiverEvent_Lost({required this.id}): super._();
  

/// The instance name of the receiver.
 final  String id;

/// Create a copy of NearbyReceiverEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$NearbyReceiverEvent_LostCopyWith<NearbyReceiverEvent_Lost> get copyWith => _$NearbyReceiverEvent_LostCopyWithImpl<NearbyReceiverEvent_Lost>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NearbyReceiverEvent_Lost&&(identical(other.id, id) || other.id == id));
}


@override
int get hashCode {
    return Object.hash(runtimeType,id);
}

@override
String toString() {
    return 'NearbyReceiverEvent.lost(id: $id)';
}


}

/// @nodoc
abstract mixin class $NearbyReceiverEvent_LostCopyWith<$Res> implements $NearbyReceiverEventCopyWith<$Res> {
  factory $NearbyReceiverEvent_LostCopyWith(NearbyReceiverEvent_Lost value, $Res Function(NearbyReceiverEvent_Lost) _then) = _$NearbyReceiverEvent_LostCopyWithImpl;
@useResult
$Res call({
 String id
});




}
/// @nodoc
class _$NearbyReceiverEvent_LostCopyWithImpl<$Res>
    implements $NearbyReceiverEvent_LostCopyWith<$Res> {
  _$NearbyReceiverEvent_LostCopyWithImpl(this._self, this._then);

  final NearbyReceiverEvent_Lost _self;
  final $Res Function(NearbyReceiverEvent_Lost) _then;

/// Create a copy of NearbyReceiverEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? id = null,}) {
  return _then(NearbyReceiverEvent_Lost(
id: null == id ? _self.id : id // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class NearbyReceiverEvent_Unavailable extends NearbyReceiverEvent {
  const NearbyReceiverEvent_Unavailable(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NearbyReceiverEvent_Unavailable);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'NearbyReceiverEvent.unavailable()';
}


}




/// @nodoc
mixin _$NearbyUiEvent {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NearbyUiEvent);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'NearbyUiEvent()';
}


}

/// @nodoc
class $NearbyUiEventCopyWith<$Res>  {
$NearbyUiEventCopyWith(NearbyUiEvent _, $Res Function(NearbyUiEvent) __);
}


/// Adds pattern-matching-related methods to [NearbyUiEvent].
extension NearbyUiEventPatterns on NearbyUiEvent {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( NearbyUiEvent_SearchStarted value)?  searchStarted,TResult Function( NearbyUiEvent_SearchStopped value)?  searchStopped,TResult Function( NearbyUiEvent_Added value)?  added,TResult Function( NearbyUiEvent_Updated value)?  updated,TResult Function( NearbyUiEvent_Removed value)?  removed,TResult Function( NearbyUiEvent_Permission value)?  permission,TResult Function( NearbyUiEvent_Unavailable value)?  unavailable,TResult Function( NearbyUiEvent_Forbidden value)?  forbidden,required TResult orElse(),}){
final _that = this;
switch (_that) {
case NearbyUiEvent_SearchStarted() when searchStarted != null:
return searchStarted(_that);case NearbyUiEvent_SearchStopped() when searchStopped != null:
return searchStopped(_that);case NearbyUiEvent_Added() when added != null:
return added(_that);case NearbyUiEvent_Updated() when updated != null:
return updated(_that);case NearbyUiEvent_Removed() when removed != null:
return removed(_that);case NearbyUiEvent_Permission() when permission != null:
return permission(_that);case NearbyUiEvent_Unavailable() when unavailable != null:
return unavailable(_that);case NearbyUiEvent_Forbidden() when forbidden != null:
return forbidden(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( NearbyUiEvent_SearchStarted value)  searchStarted,required TResult Function( NearbyUiEvent_SearchStopped value)  searchStopped,required TResult Function( NearbyUiEvent_Added value)  added,required TResult Function( NearbyUiEvent_Updated value)  updated,required TResult Function( NearbyUiEvent_Removed value)  removed,required TResult Function( NearbyUiEvent_Permission value)  permission,required TResult Function( NearbyUiEvent_Unavailable value)  unavailable,required TResult Function( NearbyUiEvent_Forbidden value)  forbidden,}){
final _that = this;
switch (_that) {
case NearbyUiEvent_SearchStarted():
return searchStarted(_that);case NearbyUiEvent_SearchStopped():
return searchStopped(_that);case NearbyUiEvent_Added():
return added(_that);case NearbyUiEvent_Updated():
return updated(_that);case NearbyUiEvent_Removed():
return removed(_that);case NearbyUiEvent_Permission():
return permission(_that);case NearbyUiEvent_Unavailable():
return unavailable(_that);case NearbyUiEvent_Forbidden():
return forbidden(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( NearbyUiEvent_SearchStarted value)?  searchStarted,TResult? Function( NearbyUiEvent_SearchStopped value)?  searchStopped,TResult? Function( NearbyUiEvent_Added value)?  added,TResult? Function( NearbyUiEvent_Updated value)?  updated,TResult? Function( NearbyUiEvent_Removed value)?  removed,TResult? Function( NearbyUiEvent_Permission value)?  permission,TResult? Function( NearbyUiEvent_Unavailable value)?  unavailable,TResult? Function( NearbyUiEvent_Forbidden value)?  forbidden,}){
final _that = this;
switch (_that) {
case NearbyUiEvent_SearchStarted() when searchStarted != null:
return searchStarted(_that);case NearbyUiEvent_SearchStopped() when searchStopped != null:
return searchStopped(_that);case NearbyUiEvent_Added() when added != null:
return added(_that);case NearbyUiEvent_Updated() when updated != null:
return updated(_that);case NearbyUiEvent_Removed() when removed != null:
return removed(_that);case NearbyUiEvent_Permission() when permission != null:
return permission(_that);case NearbyUiEvent_Unavailable() when unavailable != null:
return unavailable(_that);case NearbyUiEvent_Forbidden() when forbidden != null:
return forbidden(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  searchStarted,TResult Function()?  searchStopped,TResult Function( NearbyScreenInfo screen)?  added,TResult Function( NearbyScreenInfo screen)?  updated,TResult Function( String id)?  removed,TResult Function( NearbyPermission state)?  permission,TResult Function()?  unavailable,TResult Function()?  forbidden,required TResult orElse(),}) {final _that = this;
switch (_that) {
case NearbyUiEvent_SearchStarted() when searchStarted != null:
return searchStarted();case NearbyUiEvent_SearchStopped() when searchStopped != null:
return searchStopped();case NearbyUiEvent_Added() when added != null:
return added(_that.screen);case NearbyUiEvent_Updated() when updated != null:
return updated(_that.screen);case NearbyUiEvent_Removed() when removed != null:
return removed(_that.id);case NearbyUiEvent_Permission() when permission != null:
return permission(_that.state);case NearbyUiEvent_Unavailable() when unavailable != null:
return unavailable();case NearbyUiEvent_Forbidden() when forbidden != null:
return forbidden();case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  searchStarted,required TResult Function()  searchStopped,required TResult Function( NearbyScreenInfo screen)  added,required TResult Function( NearbyScreenInfo screen)  updated,required TResult Function( String id)  removed,required TResult Function( NearbyPermission state)  permission,required TResult Function()  unavailable,required TResult Function()  forbidden,}) {final _that = this;
switch (_that) {
case NearbyUiEvent_SearchStarted():
return searchStarted();case NearbyUiEvent_SearchStopped():
return searchStopped();case NearbyUiEvent_Added():
return added(_that.screen);case NearbyUiEvent_Updated():
return updated(_that.screen);case NearbyUiEvent_Removed():
return removed(_that.id);case NearbyUiEvent_Permission():
return permission(_that.state);case NearbyUiEvent_Unavailable():
return unavailable();case NearbyUiEvent_Forbidden():
return forbidden();}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  searchStarted,TResult? Function()?  searchStopped,TResult? Function( NearbyScreenInfo screen)?  added,TResult? Function( NearbyScreenInfo screen)?  updated,TResult? Function( String id)?  removed,TResult? Function( NearbyPermission state)?  permission,TResult? Function()?  unavailable,TResult? Function()?  forbidden,}) {final _that = this;
switch (_that) {
case NearbyUiEvent_SearchStarted() when searchStarted != null:
return searchStarted();case NearbyUiEvent_SearchStopped() when searchStopped != null:
return searchStopped();case NearbyUiEvent_Added() when added != null:
return added(_that.screen);case NearbyUiEvent_Updated() when updated != null:
return updated(_that.screen);case NearbyUiEvent_Removed() when removed != null:
return removed(_that.id);case NearbyUiEvent_Permission() when permission != null:
return permission(_that.state);case NearbyUiEvent_Unavailable() when unavailable != null:
return unavailable();case NearbyUiEvent_Forbidden() when forbidden != null:
return forbidden();case _:
  return null;

}
}

}

/// @nodoc


class NearbyUiEvent_SearchStarted extends NearbyUiEvent {
  const NearbyUiEvent_SearchStarted(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NearbyUiEvent_SearchStarted);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'NearbyUiEvent.searchStarted()';
}


}




/// @nodoc


class NearbyUiEvent_SearchStopped extends NearbyUiEvent {
  const NearbyUiEvent_SearchStopped(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NearbyUiEvent_SearchStopped);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'NearbyUiEvent.searchStopped()';
}


}




/// @nodoc


class NearbyUiEvent_Added extends NearbyUiEvent {
  const NearbyUiEvent_Added({required this.screen}): super._();
  

/// The screen.
 final  NearbyScreenInfo screen;

/// Create a copy of NearbyUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$NearbyUiEvent_AddedCopyWith<NearbyUiEvent_Added> get copyWith => _$NearbyUiEvent_AddedCopyWithImpl<NearbyUiEvent_Added>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NearbyUiEvent_Added&&(identical(other.screen, screen) || other.screen == screen));
}


@override
int get hashCode {
    return Object.hash(runtimeType,screen);
}

@override
String toString() {
    return 'NearbyUiEvent.added(screen: $screen)';
}


}

/// @nodoc
abstract mixin class $NearbyUiEvent_AddedCopyWith<$Res> implements $NearbyUiEventCopyWith<$Res> {
  factory $NearbyUiEvent_AddedCopyWith(NearbyUiEvent_Added value, $Res Function(NearbyUiEvent_Added) _then) = _$NearbyUiEvent_AddedCopyWithImpl;
@useResult
$Res call({
 NearbyScreenInfo screen
});




}
/// @nodoc
class _$NearbyUiEvent_AddedCopyWithImpl<$Res>
    implements $NearbyUiEvent_AddedCopyWith<$Res> {
  _$NearbyUiEvent_AddedCopyWithImpl(this._self, this._then);

  final NearbyUiEvent_Added _self;
  final $Res Function(NearbyUiEvent_Added) _then;

/// Create a copy of NearbyUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? screen = null,}) {
  return _then(NearbyUiEvent_Added(
screen: null == screen ? _self.screen : screen // ignore: cast_nullable_to_non_nullable
as NearbyScreenInfo,
  ));
}


}

/// @nodoc


class NearbyUiEvent_Updated extends NearbyUiEvent {
  const NearbyUiEvent_Updated({required this.screen}): super._();
  

/// The screen.
 final  NearbyScreenInfo screen;

/// Create a copy of NearbyUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$NearbyUiEvent_UpdatedCopyWith<NearbyUiEvent_Updated> get copyWith => _$NearbyUiEvent_UpdatedCopyWithImpl<NearbyUiEvent_Updated>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NearbyUiEvent_Updated&&(identical(other.screen, screen) || other.screen == screen));
}


@override
int get hashCode {
    return Object.hash(runtimeType,screen);
}

@override
String toString() {
    return 'NearbyUiEvent.updated(screen: $screen)';
}


}

/// @nodoc
abstract mixin class $NearbyUiEvent_UpdatedCopyWith<$Res> implements $NearbyUiEventCopyWith<$Res> {
  factory $NearbyUiEvent_UpdatedCopyWith(NearbyUiEvent_Updated value, $Res Function(NearbyUiEvent_Updated) _then) = _$NearbyUiEvent_UpdatedCopyWithImpl;
@useResult
$Res call({
 NearbyScreenInfo screen
});




}
/// @nodoc
class _$NearbyUiEvent_UpdatedCopyWithImpl<$Res>
    implements $NearbyUiEvent_UpdatedCopyWith<$Res> {
  _$NearbyUiEvent_UpdatedCopyWithImpl(this._self, this._then);

  final NearbyUiEvent_Updated _self;
  final $Res Function(NearbyUiEvent_Updated) _then;

/// Create a copy of NearbyUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? screen = null,}) {
  return _then(NearbyUiEvent_Updated(
screen: null == screen ? _self.screen : screen // ignore: cast_nullable_to_non_nullable
as NearbyScreenInfo,
  ));
}


}

/// @nodoc


class NearbyUiEvent_Removed extends NearbyUiEvent {
  const NearbyUiEvent_Removed({required this.id}): super._();
  

/// The id of the screen.
 final  String id;

/// Create a copy of NearbyUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$NearbyUiEvent_RemovedCopyWith<NearbyUiEvent_Removed> get copyWith => _$NearbyUiEvent_RemovedCopyWithImpl<NearbyUiEvent_Removed>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NearbyUiEvent_Removed&&(identical(other.id, id) || other.id == id));
}


@override
int get hashCode {
    return Object.hash(runtimeType,id);
}

@override
String toString() {
    return 'NearbyUiEvent.removed(id: $id)';
}


}

/// @nodoc
abstract mixin class $NearbyUiEvent_RemovedCopyWith<$Res> implements $NearbyUiEventCopyWith<$Res> {
  factory $NearbyUiEvent_RemovedCopyWith(NearbyUiEvent_Removed value, $Res Function(NearbyUiEvent_Removed) _then) = _$NearbyUiEvent_RemovedCopyWithImpl;
@useResult
$Res call({
 String id
});




}
/// @nodoc
class _$NearbyUiEvent_RemovedCopyWithImpl<$Res>
    implements $NearbyUiEvent_RemovedCopyWith<$Res> {
  _$NearbyUiEvent_RemovedCopyWithImpl(this._self, this._then);

  final NearbyUiEvent_Removed _self;
  final $Res Function(NearbyUiEvent_Removed) _then;

/// Create a copy of NearbyUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? id = null,}) {
  return _then(NearbyUiEvent_Removed(
id: null == id ? _self.id : id // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class NearbyUiEvent_Permission extends NearbyUiEvent {
  const NearbyUiEvent_Permission({required this.state}): super._();
  

/// The new state.
 final  NearbyPermission state;

/// Create a copy of NearbyUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$NearbyUiEvent_PermissionCopyWith<NearbyUiEvent_Permission> get copyWith => _$NearbyUiEvent_PermissionCopyWithImpl<NearbyUiEvent_Permission>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NearbyUiEvent_Permission&&(identical(other.state, state) || other.state == state));
}


@override
int get hashCode {
    return Object.hash(runtimeType,state);
}

@override
String toString() {
    return 'NearbyUiEvent.permission(state: $state)';
}


}

/// @nodoc
abstract mixin class $NearbyUiEvent_PermissionCopyWith<$Res> implements $NearbyUiEventCopyWith<$Res> {
  factory $NearbyUiEvent_PermissionCopyWith(NearbyUiEvent_Permission value, $Res Function(NearbyUiEvent_Permission) _then) = _$NearbyUiEvent_PermissionCopyWithImpl;
@useResult
$Res call({
 NearbyPermission state
});




}
/// @nodoc
class _$NearbyUiEvent_PermissionCopyWithImpl<$Res>
    implements $NearbyUiEvent_PermissionCopyWith<$Res> {
  _$NearbyUiEvent_PermissionCopyWithImpl(this._self, this._then);

  final NearbyUiEvent_Permission _self;
  final $Res Function(NearbyUiEvent_Permission) _then;

/// Create a copy of NearbyUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? state = null,}) {
  return _then(NearbyUiEvent_Permission(
state: null == state ? _self.state : state // ignore: cast_nullable_to_non_nullable
as NearbyPermission,
  ));
}


}

/// @nodoc


class NearbyUiEvent_Unavailable extends NearbyUiEvent {
  const NearbyUiEvent_Unavailable(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NearbyUiEvent_Unavailable);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'NearbyUiEvent.unavailable()';
}


}




/// @nodoc


class NearbyUiEvent_Forbidden extends NearbyUiEvent {
  const NearbyUiEvent_Forbidden(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NearbyUiEvent_Forbidden);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'NearbyUiEvent.forbidden()';
}


}




/// @nodoc
mixin _$PresenterUiEvent {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is PresenterUiEvent);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'PresenterUiEvent()';
}


}

/// @nodoc
class $PresenterUiEventCopyWith<$Res>  {
$PresenterUiEventCopyWith(PresenterUiEvent _, $Res Function(PresenterUiEvent) __);
}


/// Adds pattern-matching-related methods to [PresenterUiEvent].
extension PresenterUiEventPatterns on PresenterUiEvent {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( PresenterUiEvent_PairingRequired value)?  pairingRequired,TResult Function( PresenterUiEvent_WaitingForApproval value)?  waitingForApproval,TResult Function( PresenterUiEvent_Joined value)?  joined,TResult Function( PresenterUiEvent_ConnectionCode value)?  connectionCode,TResult Function( PresenterUiEvent_Page value)?  page,TResult Function( PresenterUiEvent_PauseState value)?  pauseState,TResult Function( PresenterUiEvent_Converting value)?  converting,TResult Function( PresenterUiEvent_ContentPrepared value)?  contentPrepared,TResult Function( PresenterUiEvent_ContentRejected value)?  contentRejected,TResult Function( PresenterUiEvent_Progress value)?  progress,TResult Function( PresenterUiEvent_ContentSent value)?  contentSent,TResult Function( PresenterUiEvent_HandoverResult value)?  handoverResult,TResult Function( PresenterUiEvent_Presenting value)?  presenting,TResult Function( PresenterUiEvent_PresentingStopped value)?  presentingStopped,TResult Function( PresenterUiEvent_Ended value)?  ended,TResult Function( PresenterUiEvent_Error value)?  error,required TResult orElse(),}){
final _that = this;
switch (_that) {
case PresenterUiEvent_PairingRequired() when pairingRequired != null:
return pairingRequired(_that);case PresenterUiEvent_WaitingForApproval() when waitingForApproval != null:
return waitingForApproval(_that);case PresenterUiEvent_Joined() when joined != null:
return joined(_that);case PresenterUiEvent_ConnectionCode() when connectionCode != null:
return connectionCode(_that);case PresenterUiEvent_Page() when page != null:
return page(_that);case PresenterUiEvent_PauseState() when pauseState != null:
return pauseState(_that);case PresenterUiEvent_Converting() when converting != null:
return converting(_that);case PresenterUiEvent_ContentPrepared() when contentPrepared != null:
return contentPrepared(_that);case PresenterUiEvent_ContentRejected() when contentRejected != null:
return contentRejected(_that);case PresenterUiEvent_Progress() when progress != null:
return progress(_that);case PresenterUiEvent_ContentSent() when contentSent != null:
return contentSent(_that);case PresenterUiEvent_HandoverResult() when handoverResult != null:
return handoverResult(_that);case PresenterUiEvent_Presenting() when presenting != null:
return presenting(_that);case PresenterUiEvent_PresentingStopped() when presentingStopped != null:
return presentingStopped(_that);case PresenterUiEvent_Ended() when ended != null:
return ended(_that);case PresenterUiEvent_Error() when error != null:
return error(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( PresenterUiEvent_PairingRequired value)  pairingRequired,required TResult Function( PresenterUiEvent_WaitingForApproval value)  waitingForApproval,required TResult Function( PresenterUiEvent_Joined value)  joined,required TResult Function( PresenterUiEvent_ConnectionCode value)  connectionCode,required TResult Function( PresenterUiEvent_Page value)  page,required TResult Function( PresenterUiEvent_PauseState value)  pauseState,required TResult Function( PresenterUiEvent_Converting value)  converting,required TResult Function( PresenterUiEvent_ContentPrepared value)  contentPrepared,required TResult Function( PresenterUiEvent_ContentRejected value)  contentRejected,required TResult Function( PresenterUiEvent_Progress value)  progress,required TResult Function( PresenterUiEvent_ContentSent value)  contentSent,required TResult Function( PresenterUiEvent_HandoverResult value)  handoverResult,required TResult Function( PresenterUiEvent_Presenting value)  presenting,required TResult Function( PresenterUiEvent_PresentingStopped value)  presentingStopped,required TResult Function( PresenterUiEvent_Ended value)  ended,required TResult Function( PresenterUiEvent_Error value)  error,}){
final _that = this;
switch (_that) {
case PresenterUiEvent_PairingRequired():
return pairingRequired(_that);case PresenterUiEvent_WaitingForApproval():
return waitingForApproval(_that);case PresenterUiEvent_Joined():
return joined(_that);case PresenterUiEvent_ConnectionCode():
return connectionCode(_that);case PresenterUiEvent_Page():
return page(_that);case PresenterUiEvent_PauseState():
return pauseState(_that);case PresenterUiEvent_Converting():
return converting(_that);case PresenterUiEvent_ContentPrepared():
return contentPrepared(_that);case PresenterUiEvent_ContentRejected():
return contentRejected(_that);case PresenterUiEvent_Progress():
return progress(_that);case PresenterUiEvent_ContentSent():
return contentSent(_that);case PresenterUiEvent_HandoverResult():
return handoverResult(_that);case PresenterUiEvent_Presenting():
return presenting(_that);case PresenterUiEvent_PresentingStopped():
return presentingStopped(_that);case PresenterUiEvent_Ended():
return ended(_that);case PresenterUiEvent_Error():
return error(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( PresenterUiEvent_PairingRequired value)?  pairingRequired,TResult? Function( PresenterUiEvent_WaitingForApproval value)?  waitingForApproval,TResult? Function( PresenterUiEvent_Joined value)?  joined,TResult? Function( PresenterUiEvent_ConnectionCode value)?  connectionCode,TResult? Function( PresenterUiEvent_Page value)?  page,TResult? Function( PresenterUiEvent_PauseState value)?  pauseState,TResult? Function( PresenterUiEvent_Converting value)?  converting,TResult? Function( PresenterUiEvent_ContentPrepared value)?  contentPrepared,TResult? Function( PresenterUiEvent_ContentRejected value)?  contentRejected,TResult? Function( PresenterUiEvent_Progress value)?  progress,TResult? Function( PresenterUiEvent_ContentSent value)?  contentSent,TResult? Function( PresenterUiEvent_HandoverResult value)?  handoverResult,TResult? Function( PresenterUiEvent_Presenting value)?  presenting,TResult? Function( PresenterUiEvent_PresentingStopped value)?  presentingStopped,TResult? Function( PresenterUiEvent_Ended value)?  ended,TResult? Function( PresenterUiEvent_Error value)?  error,}){
final _that = this;
switch (_that) {
case PresenterUiEvent_PairingRequired() when pairingRequired != null:
return pairingRequired(_that);case PresenterUiEvent_WaitingForApproval() when waitingForApproval != null:
return waitingForApproval(_that);case PresenterUiEvent_Joined() when joined != null:
return joined(_that);case PresenterUiEvent_ConnectionCode() when connectionCode != null:
return connectionCode(_that);case PresenterUiEvent_Page() when page != null:
return page(_that);case PresenterUiEvent_PauseState() when pauseState != null:
return pauseState(_that);case PresenterUiEvent_Converting() when converting != null:
return converting(_that);case PresenterUiEvent_ContentPrepared() when contentPrepared != null:
return contentPrepared(_that);case PresenterUiEvent_ContentRejected() when contentRejected != null:
return contentRejected(_that);case PresenterUiEvent_Progress() when progress != null:
return progress(_that);case PresenterUiEvent_ContentSent() when contentSent != null:
return contentSent(_that);case PresenterUiEvent_HandoverResult() when handoverResult != null:
return handoverResult(_that);case PresenterUiEvent_Presenting() when presenting != null:
return presenting(_that);case PresenterUiEvent_PresentingStopped() when presentingStopped != null:
return presentingStopped(_that);case PresenterUiEvent_Ended() when ended != null:
return ended(_that);case PresenterUiEvent_Error() when error != null:
return error(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  pairingRequired,TResult Function( int timeoutSecs,  int check,  String? roomName)?  waitingForApproval,TResult Function( String? roomName)?  joined,TResult Function( String sas)?  connectionCode,TResult Function( int page,  int pages)?  page,TResult Function( bool presenter,  bool display)?  pauseState,TResult Function( String name)?  converting,TResult Function( String name,  UiContentKind kind,  BigInt size,  int? pages,  String? host)?  contentPrepared,TResult Function( UiSendRejection reason)?  contentRejected,TResult Function( BigInt sent,  BigInt total)?  progress,TResult Function()?  contentSent,TResult Function( bool ok)?  handoverResult,TResult Function()?  presenting,TResult Function()?  presentingStopped,TResult Function( String code)?  ended,TResult Function( String code,  String message)?  error,required TResult orElse(),}) {final _that = this;
switch (_that) {
case PresenterUiEvent_PairingRequired() when pairingRequired != null:
return pairingRequired();case PresenterUiEvent_WaitingForApproval() when waitingForApproval != null:
return waitingForApproval(_that.timeoutSecs,_that.check,_that.roomName);case PresenterUiEvent_Joined() when joined != null:
return joined(_that.roomName);case PresenterUiEvent_ConnectionCode() when connectionCode != null:
return connectionCode(_that.sas);case PresenterUiEvent_Page() when page != null:
return page(_that.page,_that.pages);case PresenterUiEvent_PauseState() when pauseState != null:
return pauseState(_that.presenter,_that.display);case PresenterUiEvent_Converting() when converting != null:
return converting(_that.name);case PresenterUiEvent_ContentPrepared() when contentPrepared != null:
return contentPrepared(_that.name,_that.kind,_that.size,_that.pages,_that.host);case PresenterUiEvent_ContentRejected() when contentRejected != null:
return contentRejected(_that.reason);case PresenterUiEvent_Progress() when progress != null:
return progress(_that.sent,_that.total);case PresenterUiEvent_ContentSent() when contentSent != null:
return contentSent();case PresenterUiEvent_HandoverResult() when handoverResult != null:
return handoverResult(_that.ok);case PresenterUiEvent_Presenting() when presenting != null:
return presenting();case PresenterUiEvent_PresentingStopped() when presentingStopped != null:
return presentingStopped();case PresenterUiEvent_Ended() when ended != null:
return ended(_that.code);case PresenterUiEvent_Error() when error != null:
return error(_that.code,_that.message);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  pairingRequired,required TResult Function( int timeoutSecs,  int check,  String? roomName)  waitingForApproval,required TResult Function( String? roomName)  joined,required TResult Function( String sas)  connectionCode,required TResult Function( int page,  int pages)  page,required TResult Function( bool presenter,  bool display)  pauseState,required TResult Function( String name)  converting,required TResult Function( String name,  UiContentKind kind,  BigInt size,  int? pages,  String? host)  contentPrepared,required TResult Function( UiSendRejection reason)  contentRejected,required TResult Function( BigInt sent,  BigInt total)  progress,required TResult Function()  contentSent,required TResult Function( bool ok)  handoverResult,required TResult Function()  presenting,required TResult Function()  presentingStopped,required TResult Function( String code)  ended,required TResult Function( String code,  String message)  error,}) {final _that = this;
switch (_that) {
case PresenterUiEvent_PairingRequired():
return pairingRequired();case PresenterUiEvent_WaitingForApproval():
return waitingForApproval(_that.timeoutSecs,_that.check,_that.roomName);case PresenterUiEvent_Joined():
return joined(_that.roomName);case PresenterUiEvent_ConnectionCode():
return connectionCode(_that.sas);case PresenterUiEvent_Page():
return page(_that.page,_that.pages);case PresenterUiEvent_PauseState():
return pauseState(_that.presenter,_that.display);case PresenterUiEvent_Converting():
return converting(_that.name);case PresenterUiEvent_ContentPrepared():
return contentPrepared(_that.name,_that.kind,_that.size,_that.pages,_that.host);case PresenterUiEvent_ContentRejected():
return contentRejected(_that.reason);case PresenterUiEvent_Progress():
return progress(_that.sent,_that.total);case PresenterUiEvent_ContentSent():
return contentSent();case PresenterUiEvent_HandoverResult():
return handoverResult(_that.ok);case PresenterUiEvent_Presenting():
return presenting();case PresenterUiEvent_PresentingStopped():
return presentingStopped();case PresenterUiEvent_Ended():
return ended(_that.code);case PresenterUiEvent_Error():
return error(_that.code,_that.message);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  pairingRequired,TResult? Function( int timeoutSecs,  int check,  String? roomName)?  waitingForApproval,TResult? Function( String? roomName)?  joined,TResult? Function( String sas)?  connectionCode,TResult? Function( int page,  int pages)?  page,TResult? Function( bool presenter,  bool display)?  pauseState,TResult? Function( String name)?  converting,TResult? Function( String name,  UiContentKind kind,  BigInt size,  int? pages,  String? host)?  contentPrepared,TResult? Function( UiSendRejection reason)?  contentRejected,TResult? Function( BigInt sent,  BigInt total)?  progress,TResult? Function()?  contentSent,TResult? Function( bool ok)?  handoverResult,TResult? Function()?  presenting,TResult? Function()?  presentingStopped,TResult? Function( String code)?  ended,TResult? Function( String code,  String message)?  error,}) {final _that = this;
switch (_that) {
case PresenterUiEvent_PairingRequired() when pairingRequired != null:
return pairingRequired();case PresenterUiEvent_WaitingForApproval() when waitingForApproval != null:
return waitingForApproval(_that.timeoutSecs,_that.check,_that.roomName);case PresenterUiEvent_Joined() when joined != null:
return joined(_that.roomName);case PresenterUiEvent_ConnectionCode() when connectionCode != null:
return connectionCode(_that.sas);case PresenterUiEvent_Page() when page != null:
return page(_that.page,_that.pages);case PresenterUiEvent_PauseState() when pauseState != null:
return pauseState(_that.presenter,_that.display);case PresenterUiEvent_Converting() when converting != null:
return converting(_that.name);case PresenterUiEvent_ContentPrepared() when contentPrepared != null:
return contentPrepared(_that.name,_that.kind,_that.size,_that.pages,_that.host);case PresenterUiEvent_ContentRejected() when contentRejected != null:
return contentRejected(_that.reason);case PresenterUiEvent_Progress() when progress != null:
return progress(_that.sent,_that.total);case PresenterUiEvent_ContentSent() when contentSent != null:
return contentSent();case PresenterUiEvent_HandoverResult() when handoverResult != null:
return handoverResult(_that.ok);case PresenterUiEvent_Presenting() when presenting != null:
return presenting();case PresenterUiEvent_PresentingStopped() when presentingStopped != null:
return presentingStopped();case PresenterUiEvent_Ended() when ended != null:
return ended(_that.code);case PresenterUiEvent_Error() when error != null:
return error(_that.code,_that.message);case _:
  return null;

}
}

}

/// @nodoc


class PresenterUiEvent_PairingRequired extends PresenterUiEvent {
  const PresenterUiEvent_PairingRequired(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is PresenterUiEvent_PairingRequired);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'PresenterUiEvent.pairingRequired()';
}


}




/// @nodoc


class PresenterUiEvent_WaitingForApproval extends PresenterUiEvent {
  const PresenterUiEvent_WaitingForApproval({required this.timeoutSecs, required this.check, this.roomName}): super._();
  

/// Seconds until the server gives up.
 final  int timeoutSecs;
/// Two-digit check number.
 final  int check;
/// The name the display gave its room, if any (decision 49: lenient).
 final  String? roomName;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$PresenterUiEvent_WaitingForApprovalCopyWith<PresenterUiEvent_WaitingForApproval> get copyWith => _$PresenterUiEvent_WaitingForApprovalCopyWithImpl<PresenterUiEvent_WaitingForApproval>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is PresenterUiEvent_WaitingForApproval&&(identical(other.timeoutSecs, timeoutSecs) || other.timeoutSecs == timeoutSecs)&&(identical(other.check, check) || other.check == check)&&(identical(other.roomName, roomName) || other.roomName == roomName));
}


@override
int get hashCode {
    return Object.hash(runtimeType,timeoutSecs,check,roomName);
}

@override
String toString() {
    return 'PresenterUiEvent.waitingForApproval(timeoutSecs: $timeoutSecs, check: $check, roomName: $roomName)';
}


}

/// @nodoc
abstract mixin class $PresenterUiEvent_WaitingForApprovalCopyWith<$Res> implements $PresenterUiEventCopyWith<$Res> {
  factory $PresenterUiEvent_WaitingForApprovalCopyWith(PresenterUiEvent_WaitingForApproval value, $Res Function(PresenterUiEvent_WaitingForApproval) _then) = _$PresenterUiEvent_WaitingForApprovalCopyWithImpl;
@useResult
$Res call({
 int timeoutSecs, int check, String? roomName
});




}
/// @nodoc
class _$PresenterUiEvent_WaitingForApprovalCopyWithImpl<$Res>
    implements $PresenterUiEvent_WaitingForApprovalCopyWith<$Res> {
  _$PresenterUiEvent_WaitingForApprovalCopyWithImpl(this._self, this._then);

  final PresenterUiEvent_WaitingForApproval _self;
  final $Res Function(PresenterUiEvent_WaitingForApproval) _then;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? timeoutSecs = null,Object? check = null,Object? roomName = freezed,}) {
  return _then(PresenterUiEvent_WaitingForApproval(
timeoutSecs: null == timeoutSecs ? _self.timeoutSecs : timeoutSecs // ignore: cast_nullable_to_non_nullable
as int,check: null == check ? _self.check : check // ignore: cast_nullable_to_non_nullable
as int,roomName: freezed == roomName ? _self.roomName : roomName // ignore: cast_nullable_to_non_nullable
as String?,
  ));
}


}

/// @nodoc


class PresenterUiEvent_Joined extends PresenterUiEvent {
  const PresenterUiEvent_Joined({this.roomName}): super._();
  

/// The name the display gave its room, if any.
 final  String? roomName;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$PresenterUiEvent_JoinedCopyWith<PresenterUiEvent_Joined> get copyWith => _$PresenterUiEvent_JoinedCopyWithImpl<PresenterUiEvent_Joined>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is PresenterUiEvent_Joined&&(identical(other.roomName, roomName) || other.roomName == roomName));
}


@override
int get hashCode {
    return Object.hash(runtimeType,roomName);
}

@override
String toString() {
    return 'PresenterUiEvent.joined(roomName: $roomName)';
}


}

/// @nodoc
abstract mixin class $PresenterUiEvent_JoinedCopyWith<$Res> implements $PresenterUiEventCopyWith<$Res> {
  factory $PresenterUiEvent_JoinedCopyWith(PresenterUiEvent_Joined value, $Res Function(PresenterUiEvent_Joined) _then) = _$PresenterUiEvent_JoinedCopyWithImpl;
@useResult
$Res call({
 String? roomName
});




}
/// @nodoc
class _$PresenterUiEvent_JoinedCopyWithImpl<$Res>
    implements $PresenterUiEvent_JoinedCopyWith<$Res> {
  _$PresenterUiEvent_JoinedCopyWithImpl(this._self, this._then);

  final PresenterUiEvent_Joined _self;
  final $Res Function(PresenterUiEvent_Joined) _then;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? roomName = freezed,}) {
  return _then(PresenterUiEvent_Joined(
roomName: freezed == roomName ? _self.roomName : roomName // ignore: cast_nullable_to_non_nullable
as String?,
  ));
}


}

/// @nodoc


class PresenterUiEvent_ConnectionCode extends PresenterUiEvent {
  const PresenterUiEvent_ConnectionCode({required this.sas}): super._();
  

/// The six digits with a space in the middle.
 final  String sas;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$PresenterUiEvent_ConnectionCodeCopyWith<PresenterUiEvent_ConnectionCode> get copyWith => _$PresenterUiEvent_ConnectionCodeCopyWithImpl<PresenterUiEvent_ConnectionCode>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is PresenterUiEvent_ConnectionCode&&(identical(other.sas, sas) || other.sas == sas));
}


@override
int get hashCode {
    return Object.hash(runtimeType,sas);
}

@override
String toString() {
    return 'PresenterUiEvent.connectionCode(sas: $sas)';
}


}

/// @nodoc
abstract mixin class $PresenterUiEvent_ConnectionCodeCopyWith<$Res> implements $PresenterUiEventCopyWith<$Res> {
  factory $PresenterUiEvent_ConnectionCodeCopyWith(PresenterUiEvent_ConnectionCode value, $Res Function(PresenterUiEvent_ConnectionCode) _then) = _$PresenterUiEvent_ConnectionCodeCopyWithImpl;
@useResult
$Res call({
 String sas
});




}
/// @nodoc
class _$PresenterUiEvent_ConnectionCodeCopyWithImpl<$Res>
    implements $PresenterUiEvent_ConnectionCodeCopyWith<$Res> {
  _$PresenterUiEvent_ConnectionCodeCopyWithImpl(this._self, this._then);

  final PresenterUiEvent_ConnectionCode _self;
  final $Res Function(PresenterUiEvent_ConnectionCode) _then;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? sas = null,}) {
  return _then(PresenterUiEvent_ConnectionCode(
sas: null == sas ? _self.sas : sas // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class PresenterUiEvent_Page extends PresenterUiEvent {
  const PresenterUiEvent_Page({required this.page, required this.pages}): super._();
  

/// The shown page, from 1.
 final  int page;
/// The page count, from 1.
 final  int pages;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$PresenterUiEvent_PageCopyWith<PresenterUiEvent_Page> get copyWith => _$PresenterUiEvent_PageCopyWithImpl<PresenterUiEvent_Page>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is PresenterUiEvent_Page&&(identical(other.page, page) || other.page == page)&&(identical(other.pages, pages) || other.pages == pages));
}


@override
int get hashCode {
    return Object.hash(runtimeType,page,pages);
}

@override
String toString() {
    return 'PresenterUiEvent.page(page: $page, pages: $pages)';
}


}

/// @nodoc
abstract mixin class $PresenterUiEvent_PageCopyWith<$Res> implements $PresenterUiEventCopyWith<$Res> {
  factory $PresenterUiEvent_PageCopyWith(PresenterUiEvent_Page value, $Res Function(PresenterUiEvent_Page) _then) = _$PresenterUiEvent_PageCopyWithImpl;
@useResult
$Res call({
 int page, int pages
});




}
/// @nodoc
class _$PresenterUiEvent_PageCopyWithImpl<$Res>
    implements $PresenterUiEvent_PageCopyWith<$Res> {
  _$PresenterUiEvent_PageCopyWithImpl(this._self, this._then);

  final PresenterUiEvent_Page _self;
  final $Res Function(PresenterUiEvent_Page) _then;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? page = null,Object? pages = null,}) {
  return _then(PresenterUiEvent_Page(
page: null == page ? _self.page : page // ignore: cast_nullable_to_non_nullable
as int,pages: null == pages ? _self.pages : pages // ignore: cast_nullable_to_non_nullable
as int,
  ));
}


}

/// @nodoc


class PresenterUiEvent_PauseState extends PresenterUiEvent {
  const PresenterUiEvent_PauseState({required this.presenter, required this.display}): super._();
  

/// The presenter's own flag, as the display holds it.
 final  bool presenter;
/// The display froze the picture on its own.
 final  bool display;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$PresenterUiEvent_PauseStateCopyWith<PresenterUiEvent_PauseState> get copyWith => _$PresenterUiEvent_PauseStateCopyWithImpl<PresenterUiEvent_PauseState>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is PresenterUiEvent_PauseState&&(identical(other.presenter, presenter) || other.presenter == presenter)&&(identical(other.display, display) || other.display == display));
}


@override
int get hashCode {
    return Object.hash(runtimeType,presenter,display);
}

@override
String toString() {
    return 'PresenterUiEvent.pauseState(presenter: $presenter, display: $display)';
}


}

/// @nodoc
abstract mixin class $PresenterUiEvent_PauseStateCopyWith<$Res> implements $PresenterUiEventCopyWith<$Res> {
  factory $PresenterUiEvent_PauseStateCopyWith(PresenterUiEvent_PauseState value, $Res Function(PresenterUiEvent_PauseState) _then) = _$PresenterUiEvent_PauseStateCopyWithImpl;
@useResult
$Res call({
 bool presenter, bool display
});




}
/// @nodoc
class _$PresenterUiEvent_PauseStateCopyWithImpl<$Res>
    implements $PresenterUiEvent_PauseStateCopyWith<$Res> {
  _$PresenterUiEvent_PauseStateCopyWithImpl(this._self, this._then);

  final PresenterUiEvent_PauseState _self;
  final $Res Function(PresenterUiEvent_PauseState) _then;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? presenter = null,Object? display = null,}) {
  return _then(PresenterUiEvent_PauseState(
presenter: null == presenter ? _self.presenter : presenter // ignore: cast_nullable_to_non_nullable
as bool,display: null == display ? _self.display : display // ignore: cast_nullable_to_non_nullable
as bool,
  ));
}


}

/// @nodoc


class PresenterUiEvent_Converting extends PresenterUiEvent {
  const PresenterUiEvent_Converting({required this.name}): super._();
  

/// The file name as picked.
 final  String name;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$PresenterUiEvent_ConvertingCopyWith<PresenterUiEvent_Converting> get copyWith => _$PresenterUiEvent_ConvertingCopyWithImpl<PresenterUiEvent_Converting>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is PresenterUiEvent_Converting&&(identical(other.name, name) || other.name == name));
}


@override
int get hashCode {
    return Object.hash(runtimeType,name);
}

@override
String toString() {
    return 'PresenterUiEvent.converting(name: $name)';
}


}

/// @nodoc
abstract mixin class $PresenterUiEvent_ConvertingCopyWith<$Res> implements $PresenterUiEventCopyWith<$Res> {
  factory $PresenterUiEvent_ConvertingCopyWith(PresenterUiEvent_Converting value, $Res Function(PresenterUiEvent_Converting) _then) = _$PresenterUiEvent_ConvertingCopyWithImpl;
@useResult
$Res call({
 String name
});




}
/// @nodoc
class _$PresenterUiEvent_ConvertingCopyWithImpl<$Res>
    implements $PresenterUiEvent_ConvertingCopyWith<$Res> {
  _$PresenterUiEvent_ConvertingCopyWithImpl(this._self, this._then);

  final PresenterUiEvent_Converting _self;
  final $Res Function(PresenterUiEvent_Converting) _then;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? name = null,}) {
  return _then(PresenterUiEvent_Converting(
name: null == name ? _self.name : name // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class PresenterUiEvent_ContentPrepared extends PresenterUiEvent {
  const PresenterUiEvent_ContentPrepared({required this.name, required this.kind, required this.size, this.pages, this.host}): super._();
  

/// File name, or the host of a link.
 final  String name;
/// Image, PDF or link.
 final  UiContentKind kind;
/// Size in bytes.
 final  BigInt size;
/// Pages of a PDF, if known.
 final  int? pages;
/// Host of a link in its punycode form.
 final  String? host;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$PresenterUiEvent_ContentPreparedCopyWith<PresenterUiEvent_ContentPrepared> get copyWith => _$PresenterUiEvent_ContentPreparedCopyWithImpl<PresenterUiEvent_ContentPrepared>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is PresenterUiEvent_ContentPrepared&&(identical(other.name, name) || other.name == name)&&(identical(other.kind, kind) || other.kind == kind)&&(identical(other.size, size) || other.size == size)&&(identical(other.pages, pages) || other.pages == pages)&&(identical(other.host, host) || other.host == host));
}


@override
int get hashCode {
    return Object.hash(runtimeType,name,kind,size,pages,host);
}

@override
String toString() {
    return 'PresenterUiEvent.contentPrepared(name: $name, kind: $kind, size: $size, pages: $pages, host: $host)';
}


}

/// @nodoc
abstract mixin class $PresenterUiEvent_ContentPreparedCopyWith<$Res> implements $PresenterUiEventCopyWith<$Res> {
  factory $PresenterUiEvent_ContentPreparedCopyWith(PresenterUiEvent_ContentPrepared value, $Res Function(PresenterUiEvent_ContentPrepared) _then) = _$PresenterUiEvent_ContentPreparedCopyWithImpl;
@useResult
$Res call({
 String name, UiContentKind kind, BigInt size, int? pages, String? host
});




}
/// @nodoc
class _$PresenterUiEvent_ContentPreparedCopyWithImpl<$Res>
    implements $PresenterUiEvent_ContentPreparedCopyWith<$Res> {
  _$PresenterUiEvent_ContentPreparedCopyWithImpl(this._self, this._then);

  final PresenterUiEvent_ContentPrepared _self;
  final $Res Function(PresenterUiEvent_ContentPrepared) _then;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? name = null,Object? kind = null,Object? size = null,Object? pages = freezed,Object? host = freezed,}) {
  return _then(PresenterUiEvent_ContentPrepared(
name: null == name ? _self.name : name // ignore: cast_nullable_to_non_nullable
as String,kind: null == kind ? _self.kind : kind // ignore: cast_nullable_to_non_nullable
as UiContentKind,size: null == size ? _self.size : size // ignore: cast_nullable_to_non_nullable
as BigInt,pages: freezed == pages ? _self.pages : pages // ignore: cast_nullable_to_non_nullable
as int?,host: freezed == host ? _self.host : host // ignore: cast_nullable_to_non_nullable
as String?,
  ));
}


}

/// @nodoc


class PresenterUiEvent_ContentRejected extends PresenterUiEvent {
  const PresenterUiEvent_ContentRejected({required this.reason}): super._();
  

/// Why.
 final  UiSendRejection reason;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$PresenterUiEvent_ContentRejectedCopyWith<PresenterUiEvent_ContentRejected> get copyWith => _$PresenterUiEvent_ContentRejectedCopyWithImpl<PresenterUiEvent_ContentRejected>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is PresenterUiEvent_ContentRejected&&(identical(other.reason, reason) || other.reason == reason));
}


@override
int get hashCode {
    return Object.hash(runtimeType,reason);
}

@override
String toString() {
    return 'PresenterUiEvent.contentRejected(reason: $reason)';
}


}

/// @nodoc
abstract mixin class $PresenterUiEvent_ContentRejectedCopyWith<$Res> implements $PresenterUiEventCopyWith<$Res> {
  factory $PresenterUiEvent_ContentRejectedCopyWith(PresenterUiEvent_ContentRejected value, $Res Function(PresenterUiEvent_ContentRejected) _then) = _$PresenterUiEvent_ContentRejectedCopyWithImpl;
@useResult
$Res call({
 UiSendRejection reason
});




}
/// @nodoc
class _$PresenterUiEvent_ContentRejectedCopyWithImpl<$Res>
    implements $PresenterUiEvent_ContentRejectedCopyWith<$Res> {
  _$PresenterUiEvent_ContentRejectedCopyWithImpl(this._self, this._then);

  final PresenterUiEvent_ContentRejected _self;
  final $Res Function(PresenterUiEvent_ContentRejected) _then;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? reason = null,}) {
  return _then(PresenterUiEvent_ContentRejected(
reason: null == reason ? _self.reason : reason // ignore: cast_nullable_to_non_nullable
as UiSendRejection,
  ));
}


}

/// @nodoc


class PresenterUiEvent_Progress extends PresenterUiEvent {
  const PresenterUiEvent_Progress({required this.sent, required this.total}): super._();
  

/// Bytes so far.
 final  BigInt sent;
/// Bytes in total.
 final  BigInt total;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$PresenterUiEvent_ProgressCopyWith<PresenterUiEvent_Progress> get copyWith => _$PresenterUiEvent_ProgressCopyWithImpl<PresenterUiEvent_Progress>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is PresenterUiEvent_Progress&&(identical(other.sent, sent) || other.sent == sent)&&(identical(other.total, total) || other.total == total));
}


@override
int get hashCode {
    return Object.hash(runtimeType,sent,total);
}

@override
String toString() {
    return 'PresenterUiEvent.progress(sent: $sent, total: $total)';
}


}

/// @nodoc
abstract mixin class $PresenterUiEvent_ProgressCopyWith<$Res> implements $PresenterUiEventCopyWith<$Res> {
  factory $PresenterUiEvent_ProgressCopyWith(PresenterUiEvent_Progress value, $Res Function(PresenterUiEvent_Progress) _then) = _$PresenterUiEvent_ProgressCopyWithImpl;
@useResult
$Res call({
 BigInt sent, BigInt total
});




}
/// @nodoc
class _$PresenterUiEvent_ProgressCopyWithImpl<$Res>
    implements $PresenterUiEvent_ProgressCopyWith<$Res> {
  _$PresenterUiEvent_ProgressCopyWithImpl(this._self, this._then);

  final PresenterUiEvent_Progress _self;
  final $Res Function(PresenterUiEvent_Progress) _then;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? sent = null,Object? total = null,}) {
  return _then(PresenterUiEvent_Progress(
sent: null == sent ? _self.sent : sent // ignore: cast_nullable_to_non_nullable
as BigInt,total: null == total ? _self.total : total // ignore: cast_nullable_to_non_nullable
as BigInt,
  ));
}


}

/// @nodoc


class PresenterUiEvent_ContentSent extends PresenterUiEvent {
  const PresenterUiEvent_ContentSent(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is PresenterUiEvent_ContentSent);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'PresenterUiEvent.contentSent()';
}


}




/// @nodoc


class PresenterUiEvent_HandoverResult extends PresenterUiEvent {
  const PresenterUiEvent_HandoverResult({required this.ok}): super._();
  

/// The display answered `handover_ok`.
 final  bool ok;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$PresenterUiEvent_HandoverResultCopyWith<PresenterUiEvent_HandoverResult> get copyWith => _$PresenterUiEvent_HandoverResultCopyWithImpl<PresenterUiEvent_HandoverResult>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is PresenterUiEvent_HandoverResult&&(identical(other.ok, ok) || other.ok == ok));
}


@override
int get hashCode {
    return Object.hash(runtimeType,ok);
}

@override
String toString() {
    return 'PresenterUiEvent.handoverResult(ok: $ok)';
}


}

/// @nodoc
abstract mixin class $PresenterUiEvent_HandoverResultCopyWith<$Res> implements $PresenterUiEventCopyWith<$Res> {
  factory $PresenterUiEvent_HandoverResultCopyWith(PresenterUiEvent_HandoverResult value, $Res Function(PresenterUiEvent_HandoverResult) _then) = _$PresenterUiEvent_HandoverResultCopyWithImpl;
@useResult
$Res call({
 bool ok
});




}
/// @nodoc
class _$PresenterUiEvent_HandoverResultCopyWithImpl<$Res>
    implements $PresenterUiEvent_HandoverResultCopyWith<$Res> {
  _$PresenterUiEvent_HandoverResultCopyWithImpl(this._self, this._then);

  final PresenterUiEvent_HandoverResult _self;
  final $Res Function(PresenterUiEvent_HandoverResult) _then;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? ok = null,}) {
  return _then(PresenterUiEvent_HandoverResult(
ok: null == ok ? _self.ok : ok // ignore: cast_nullable_to_non_nullable
as bool,
  ));
}


}

/// @nodoc


class PresenterUiEvent_Presenting extends PresenterUiEvent {
  const PresenterUiEvent_Presenting(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is PresenterUiEvent_Presenting);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'PresenterUiEvent.presenting()';
}


}




/// @nodoc


class PresenterUiEvent_PresentingStopped extends PresenterUiEvent {
  const PresenterUiEvent_PresentingStopped(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is PresenterUiEvent_PresentingStopped);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'PresenterUiEvent.presentingStopped()';
}


}




/// @nodoc


class PresenterUiEvent_Ended extends PresenterUiEvent {
  const PresenterUiEvent_Ended({required this.code}): super._();
  

/// `removed_by_host` or `display_left`.
 final  String code;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$PresenterUiEvent_EndedCopyWith<PresenterUiEvent_Ended> get copyWith => _$PresenterUiEvent_EndedCopyWithImpl<PresenterUiEvent_Ended>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is PresenterUiEvent_Ended&&(identical(other.code, code) || other.code == code));
}


@override
int get hashCode {
    return Object.hash(runtimeType,code);
}

@override
String toString() {
    return 'PresenterUiEvent.ended(code: $code)';
}


}

/// @nodoc
abstract mixin class $PresenterUiEvent_EndedCopyWith<$Res> implements $PresenterUiEventCopyWith<$Res> {
  factory $PresenterUiEvent_EndedCopyWith(PresenterUiEvent_Ended value, $Res Function(PresenterUiEvent_Ended) _then) = _$PresenterUiEvent_EndedCopyWithImpl;
@useResult
$Res call({
 String code
});




}
/// @nodoc
class _$PresenterUiEvent_EndedCopyWithImpl<$Res>
    implements $PresenterUiEvent_EndedCopyWith<$Res> {
  _$PresenterUiEvent_EndedCopyWithImpl(this._self, this._then);

  final PresenterUiEvent_Ended _self;
  final $Res Function(PresenterUiEvent_Ended) _then;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? code = null,}) {
  return _then(PresenterUiEvent_Ended(
code: null == code ? _self.code : code // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class PresenterUiEvent_Error extends PresenterUiEvent {
  const PresenterUiEvent_Error({required this.code, required this.message}): super._();
  

/// The code the app maps.
 final  String code;
/// Technical message in English (logs only).
 final  String message;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$PresenterUiEvent_ErrorCopyWith<PresenterUiEvent_Error> get copyWith => _$PresenterUiEvent_ErrorCopyWithImpl<PresenterUiEvent_Error>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is PresenterUiEvent_Error&&(identical(other.code, code) || other.code == code)&&(identical(other.message, message) || other.message == message));
}


@override
int get hashCode {
    return Object.hash(runtimeType,code,message);
}

@override
String toString() {
    return 'PresenterUiEvent.error(code: $code, message: $message)';
}


}

/// @nodoc
abstract mixin class $PresenterUiEvent_ErrorCopyWith<$Res> implements $PresenterUiEventCopyWith<$Res> {
  factory $PresenterUiEvent_ErrorCopyWith(PresenterUiEvent_Error value, $Res Function(PresenterUiEvent_Error) _then) = _$PresenterUiEvent_ErrorCopyWithImpl;
@useResult
$Res call({
 String code, String message
});




}
/// @nodoc
class _$PresenterUiEvent_ErrorCopyWithImpl<$Res>
    implements $PresenterUiEvent_ErrorCopyWith<$Res> {
  _$PresenterUiEvent_ErrorCopyWithImpl(this._self, this._then);

  final PresenterUiEvent_Error _self;
  final $Res Function(PresenterUiEvent_Error) _then;

/// Create a copy of PresenterUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? code = null,Object? message = null,}) {
  return _then(PresenterUiEvent_Error(
code: null == code ? _self.code : code // ignore: cast_nullable_to_non_nullable
as String,message: null == message ? _self.message : message // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc
mixin _$ReceiverUiEvent {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ReceiverUiEvent()';
}


}

/// @nodoc
class $ReceiverUiEventCopyWith<$Res>  {
$ReceiverUiEventCopyWith(ReceiverUiEvent _, $Res Function(ReceiverUiEvent) __);
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( ReceiverUiEvent_RoomCode value)?  roomCode,TResult Function( ReceiverUiEvent_PeerJoined value)?  peerJoined,TResult Function( ReceiverUiEvent_HandoverStored value)?  handoverStored,TResult Function( ReceiverUiEvent_HandoverExpired value)?  handoverExpired,TResult Function( ReceiverUiEvent_JoinRequest value)?  joinRequest,TResult Function( ReceiverUiEvent_JoinRequestCancelled value)?  joinRequestCancelled,TResult Function( ReceiverUiEvent_JoinModeChanged value)?  joinModeChanged,TResult Function( ReceiverUiEvent_Blocked value)?  blocked,TResult Function( ReceiverUiEvent_Unblocked value)?  unblocked,TResult Function( ReceiverUiEvent_ServerNotice value)?  serverNotice,TResult Function( ReceiverUiEvent_RoomEnded value)?  roomEnded,TResult Function( ReceiverUiEvent_ConnectionState value)?  connectionState,TResult Function( ReceiverUiEvent_VideoTrack value)?  videoTrack,TResult Function( ReceiverUiEvent_Stats value)?  stats,TResult Function( ReceiverUiEvent_FrameReady value)?  frameReady,TResult Function( ReceiverUiEvent_EncodedReady value)?  encodedReady,TResult Function( ReceiverUiEvent_VideoStats value)?  videoStats,TResult Function( ReceiverUiEvent_ContentTransferStarted value)?  contentTransferStarted,TResult Function( ReceiverUiEvent_ContentTransferProgress value)?  contentTransferProgress,TResult Function( ReceiverUiEvent_ContentFrameReady value)?  contentFrameReady,TResult Function( ReceiverUiEvent_ContentDocumentOpened value)?  contentDocumentOpened,TResult Function( ReceiverUiEvent_ContentLink value)?  contentLink,TResult Function( ReceiverUiEvent_ContentStopped value)?  contentStopped,TResult Function( ReceiverUiEvent_ContentError value)?  contentError,TResult Function( ReceiverUiEvent_PeerLeft value)?  peerLeft,TResult Function( ReceiverUiEvent_PresentationStopped value)?  presentationStopped,TResult Function( ReceiverUiEvent_PauseChanged value)?  pauseChanged,TResult Function( ReceiverUiEvent_ConnectionCode value)?  connectionCode,TResult Function( ReceiverUiEvent_Error value)?  error,required TResult orElse(),}){
final _that = this;
switch (_that) {
case ReceiverUiEvent_RoomCode() when roomCode != null:
return roomCode(_that);case ReceiverUiEvent_PeerJoined() when peerJoined != null:
return peerJoined(_that);case ReceiverUiEvent_HandoverStored() when handoverStored != null:
return handoverStored(_that);case ReceiverUiEvent_HandoverExpired() when handoverExpired != null:
return handoverExpired(_that);case ReceiverUiEvent_JoinRequest() when joinRequest != null:
return joinRequest(_that);case ReceiverUiEvent_JoinRequestCancelled() when joinRequestCancelled != null:
return joinRequestCancelled(_that);case ReceiverUiEvent_JoinModeChanged() when joinModeChanged != null:
return joinModeChanged(_that);case ReceiverUiEvent_Blocked() when blocked != null:
return blocked(_that);case ReceiverUiEvent_Unblocked() when unblocked != null:
return unblocked(_that);case ReceiverUiEvent_ServerNotice() when serverNotice != null:
return serverNotice(_that);case ReceiverUiEvent_RoomEnded() when roomEnded != null:
return roomEnded(_that);case ReceiverUiEvent_ConnectionState() when connectionState != null:
return connectionState(_that);case ReceiverUiEvent_VideoTrack() when videoTrack != null:
return videoTrack(_that);case ReceiverUiEvent_Stats() when stats != null:
return stats(_that);case ReceiverUiEvent_FrameReady() when frameReady != null:
return frameReady(_that);case ReceiverUiEvent_EncodedReady() when encodedReady != null:
return encodedReady(_that);case ReceiverUiEvent_VideoStats() when videoStats != null:
return videoStats(_that);case ReceiverUiEvent_ContentTransferStarted() when contentTransferStarted != null:
return contentTransferStarted(_that);case ReceiverUiEvent_ContentTransferProgress() when contentTransferProgress != null:
return contentTransferProgress(_that);case ReceiverUiEvent_ContentFrameReady() when contentFrameReady != null:
return contentFrameReady(_that);case ReceiverUiEvent_ContentDocumentOpened() when contentDocumentOpened != null:
return contentDocumentOpened(_that);case ReceiverUiEvent_ContentLink() when contentLink != null:
return contentLink(_that);case ReceiverUiEvent_ContentStopped() when contentStopped != null:
return contentStopped(_that);case ReceiverUiEvent_ContentError() when contentError != null:
return contentError(_that);case ReceiverUiEvent_PeerLeft() when peerLeft != null:
return peerLeft(_that);case ReceiverUiEvent_PresentationStopped() when presentationStopped != null:
return presentationStopped(_that);case ReceiverUiEvent_PauseChanged() when pauseChanged != null:
return pauseChanged(_that);case ReceiverUiEvent_ConnectionCode() when connectionCode != null:
return connectionCode(_that);case ReceiverUiEvent_Error() when error != null:
return error(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( ReceiverUiEvent_RoomCode value)  roomCode,required TResult Function( ReceiverUiEvent_PeerJoined value)  peerJoined,required TResult Function( ReceiverUiEvent_HandoverStored value)  handoverStored,required TResult Function( ReceiverUiEvent_HandoverExpired value)  handoverExpired,required TResult Function( ReceiverUiEvent_JoinRequest value)  joinRequest,required TResult Function( ReceiverUiEvent_JoinRequestCancelled value)  joinRequestCancelled,required TResult Function( ReceiverUiEvent_JoinModeChanged value)  joinModeChanged,required TResult Function( ReceiverUiEvent_Blocked value)  blocked,required TResult Function( ReceiverUiEvent_Unblocked value)  unblocked,required TResult Function( ReceiverUiEvent_ServerNotice value)  serverNotice,required TResult Function( ReceiverUiEvent_RoomEnded value)  roomEnded,required TResult Function( ReceiverUiEvent_ConnectionState value)  connectionState,required TResult Function( ReceiverUiEvent_VideoTrack value)  videoTrack,required TResult Function( ReceiverUiEvent_Stats value)  stats,required TResult Function( ReceiverUiEvent_FrameReady value)  frameReady,required TResult Function( ReceiverUiEvent_EncodedReady value)  encodedReady,required TResult Function( ReceiverUiEvent_VideoStats value)  videoStats,required TResult Function( ReceiverUiEvent_ContentTransferStarted value)  contentTransferStarted,required TResult Function( ReceiverUiEvent_ContentTransferProgress value)  contentTransferProgress,required TResult Function( ReceiverUiEvent_ContentFrameReady value)  contentFrameReady,required TResult Function( ReceiverUiEvent_ContentDocumentOpened value)  contentDocumentOpened,required TResult Function( ReceiverUiEvent_ContentLink value)  contentLink,required TResult Function( ReceiverUiEvent_ContentStopped value)  contentStopped,required TResult Function( ReceiverUiEvent_ContentError value)  contentError,required TResult Function( ReceiverUiEvent_PeerLeft value)  peerLeft,required TResult Function( ReceiverUiEvent_PresentationStopped value)  presentationStopped,required TResult Function( ReceiverUiEvent_PauseChanged value)  pauseChanged,required TResult Function( ReceiverUiEvent_ConnectionCode value)  connectionCode,required TResult Function( ReceiverUiEvent_Error value)  error,}){
final _that = this;
switch (_that) {
case ReceiverUiEvent_RoomCode():
return roomCode(_that);case ReceiverUiEvent_PeerJoined():
return peerJoined(_that);case ReceiverUiEvent_HandoverStored():
return handoverStored(_that);case ReceiverUiEvent_HandoverExpired():
return handoverExpired(_that);case ReceiverUiEvent_JoinRequest():
return joinRequest(_that);case ReceiverUiEvent_JoinRequestCancelled():
return joinRequestCancelled(_that);case ReceiverUiEvent_JoinModeChanged():
return joinModeChanged(_that);case ReceiverUiEvent_Blocked():
return blocked(_that);case ReceiverUiEvent_Unblocked():
return unblocked(_that);case ReceiverUiEvent_ServerNotice():
return serverNotice(_that);case ReceiverUiEvent_RoomEnded():
return roomEnded(_that);case ReceiverUiEvent_ConnectionState():
return connectionState(_that);case ReceiverUiEvent_VideoTrack():
return videoTrack(_that);case ReceiverUiEvent_Stats():
return stats(_that);case ReceiverUiEvent_FrameReady():
return frameReady(_that);case ReceiverUiEvent_EncodedReady():
return encodedReady(_that);case ReceiverUiEvent_VideoStats():
return videoStats(_that);case ReceiverUiEvent_ContentTransferStarted():
return contentTransferStarted(_that);case ReceiverUiEvent_ContentTransferProgress():
return contentTransferProgress(_that);case ReceiverUiEvent_ContentFrameReady():
return contentFrameReady(_that);case ReceiverUiEvent_ContentDocumentOpened():
return contentDocumentOpened(_that);case ReceiverUiEvent_ContentLink():
return contentLink(_that);case ReceiverUiEvent_ContentStopped():
return contentStopped(_that);case ReceiverUiEvent_ContentError():
return contentError(_that);case ReceiverUiEvent_PeerLeft():
return peerLeft(_that);case ReceiverUiEvent_PresentationStopped():
return presentationStopped(_that);case ReceiverUiEvent_PauseChanged():
return pauseChanged(_that);case ReceiverUiEvent_ConnectionCode():
return connectionCode(_that);case ReceiverUiEvent_Error():
return error(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( ReceiverUiEvent_RoomCode value)?  roomCode,TResult? Function( ReceiverUiEvent_PeerJoined value)?  peerJoined,TResult? Function( ReceiverUiEvent_HandoverStored value)?  handoverStored,TResult? Function( ReceiverUiEvent_HandoverExpired value)?  handoverExpired,TResult? Function( ReceiverUiEvent_JoinRequest value)?  joinRequest,TResult? Function( ReceiverUiEvent_JoinRequestCancelled value)?  joinRequestCancelled,TResult? Function( ReceiverUiEvent_JoinModeChanged value)?  joinModeChanged,TResult? Function( ReceiverUiEvent_Blocked value)?  blocked,TResult? Function( ReceiverUiEvent_Unblocked value)?  unblocked,TResult? Function( ReceiverUiEvent_ServerNotice value)?  serverNotice,TResult? Function( ReceiverUiEvent_RoomEnded value)?  roomEnded,TResult? Function( ReceiverUiEvent_ConnectionState value)?  connectionState,TResult? Function( ReceiverUiEvent_VideoTrack value)?  videoTrack,TResult? Function( ReceiverUiEvent_Stats value)?  stats,TResult? Function( ReceiverUiEvent_FrameReady value)?  frameReady,TResult? Function( ReceiverUiEvent_EncodedReady value)?  encodedReady,TResult? Function( ReceiverUiEvent_VideoStats value)?  videoStats,TResult? Function( ReceiverUiEvent_ContentTransferStarted value)?  contentTransferStarted,TResult? Function( ReceiverUiEvent_ContentTransferProgress value)?  contentTransferProgress,TResult? Function( ReceiverUiEvent_ContentFrameReady value)?  contentFrameReady,TResult? Function( ReceiverUiEvent_ContentDocumentOpened value)?  contentDocumentOpened,TResult? Function( ReceiverUiEvent_ContentLink value)?  contentLink,TResult? Function( ReceiverUiEvent_ContentStopped value)?  contentStopped,TResult? Function( ReceiverUiEvent_ContentError value)?  contentError,TResult? Function( ReceiverUiEvent_PeerLeft value)?  peerLeft,TResult? Function( ReceiverUiEvent_PresentationStopped value)?  presentationStopped,TResult? Function( ReceiverUiEvent_PauseChanged value)?  pauseChanged,TResult? Function( ReceiverUiEvent_ConnectionCode value)?  connectionCode,TResult? Function( ReceiverUiEvent_Error value)?  error,}){
final _that = this;
switch (_that) {
case ReceiverUiEvent_RoomCode() when roomCode != null:
return roomCode(_that);case ReceiverUiEvent_PeerJoined() when peerJoined != null:
return peerJoined(_that);case ReceiverUiEvent_HandoverStored() when handoverStored != null:
return handoverStored(_that);case ReceiverUiEvent_HandoverExpired() when handoverExpired != null:
return handoverExpired(_that);case ReceiverUiEvent_JoinRequest() when joinRequest != null:
return joinRequest(_that);case ReceiverUiEvent_JoinRequestCancelled() when joinRequestCancelled != null:
return joinRequestCancelled(_that);case ReceiverUiEvent_JoinModeChanged() when joinModeChanged != null:
return joinModeChanged(_that);case ReceiverUiEvent_Blocked() when blocked != null:
return blocked(_that);case ReceiverUiEvent_Unblocked() when unblocked != null:
return unblocked(_that);case ReceiverUiEvent_ServerNotice() when serverNotice != null:
return serverNotice(_that);case ReceiverUiEvent_RoomEnded() when roomEnded != null:
return roomEnded(_that);case ReceiverUiEvent_ConnectionState() when connectionState != null:
return connectionState(_that);case ReceiverUiEvent_VideoTrack() when videoTrack != null:
return videoTrack(_that);case ReceiverUiEvent_Stats() when stats != null:
return stats(_that);case ReceiverUiEvent_FrameReady() when frameReady != null:
return frameReady(_that);case ReceiverUiEvent_EncodedReady() when encodedReady != null:
return encodedReady(_that);case ReceiverUiEvent_VideoStats() when videoStats != null:
return videoStats(_that);case ReceiverUiEvent_ContentTransferStarted() when contentTransferStarted != null:
return contentTransferStarted(_that);case ReceiverUiEvent_ContentTransferProgress() when contentTransferProgress != null:
return contentTransferProgress(_that);case ReceiverUiEvent_ContentFrameReady() when contentFrameReady != null:
return contentFrameReady(_that);case ReceiverUiEvent_ContentDocumentOpened() when contentDocumentOpened != null:
return contentDocumentOpened(_that);case ReceiverUiEvent_ContentLink() when contentLink != null:
return contentLink(_that);case ReceiverUiEvent_ContentStopped() when contentStopped != null:
return contentStopped(_that);case ReceiverUiEvent_ContentError() when contentError != null:
return contentError(_that);case ReceiverUiEvent_PeerLeft() when peerLeft != null:
return peerLeft(_that);case ReceiverUiEvent_PresentationStopped() when presentationStopped != null:
return presentationStopped(_that);case ReceiverUiEvent_PauseChanged() when pauseChanged != null:
return pauseChanged(_that);case ReceiverUiEvent_ConnectionCode() when connectionCode != null:
return connectionCode(_that);case ReceiverUiEvent_Error() when error != null:
return error(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String code,  UiJoinMode joinMode)?  roomCode,TResult Function( String name,  bool handover)?  peerJoined,TResult Function( String name)?  handoverStored,TResult Function()?  handoverExpired,TResult Function( BigInt id,  String name,  int check)?  joinRequest,TResult Function( BigInt id,  String reason)?  joinRequestCancelled,TResult Function( UiJoinMode mode)?  joinModeChanged,TResult Function( BigInt id,  String name)?  blocked,TResult Function( BigInt id)?  unblocked,TResult Function( String code,  String message)?  serverNotice,TResult Function()?  roomEnded,TResult Function( UiConnectionState state)?  connectionState,TResult Function( String mime,  String? codec)?  videoTrack,TResult Function( BigInt packets,  BigInt bytes,  BigInt frames,  BigInt keyframes)?  stats,TResult Function()?  frameReady,TResult Function()?  encodedReady,TResult Function( BigInt decoded,  BigInt delivered,  BigInt dropped,  BigInt droppedPackets,  BigInt decodeErrors,  BigInt keyframeRequests,  int width,  int height,  BigInt avgDecodeUs,  String? error)?  videoStats,TResult Function( bool document,  String name,  BigInt size)?  contentTransferStarted,TResult Function( BigInt received,  BigInt total)?  contentTransferProgress,TResult Function()?  contentFrameReady,TResult Function( String name,  int pages)?  contentDocumentOpened,TResult Function( String url,  String host)?  contentLink,TResult Function()?  contentStopped,TResult Function( UiContentError error)?  contentError,TResult Function()?  peerLeft,TResult Function()?  presentationStopped,TResult Function( bool presenter,  bool display)?  pauseChanged,TResult Function( String sas)?  connectionCode,TResult Function( String code,  String message)?  error,required TResult orElse(),}) {final _that = this;
switch (_that) {
case ReceiverUiEvent_RoomCode() when roomCode != null:
return roomCode(_that.code,_that.joinMode);case ReceiverUiEvent_PeerJoined() when peerJoined != null:
return peerJoined(_that.name,_that.handover);case ReceiverUiEvent_HandoverStored() when handoverStored != null:
return handoverStored(_that.name);case ReceiverUiEvent_HandoverExpired() when handoverExpired != null:
return handoverExpired();case ReceiverUiEvent_JoinRequest() when joinRequest != null:
return joinRequest(_that.id,_that.name,_that.check);case ReceiverUiEvent_JoinRequestCancelled() when joinRequestCancelled != null:
return joinRequestCancelled(_that.id,_that.reason);case ReceiverUiEvent_JoinModeChanged() when joinModeChanged != null:
return joinModeChanged(_that.mode);case ReceiverUiEvent_Blocked() when blocked != null:
return blocked(_that.id,_that.name);case ReceiverUiEvent_Unblocked() when unblocked != null:
return unblocked(_that.id);case ReceiverUiEvent_ServerNotice() when serverNotice != null:
return serverNotice(_that.code,_that.message);case ReceiverUiEvent_RoomEnded() when roomEnded != null:
return roomEnded();case ReceiverUiEvent_ConnectionState() when connectionState != null:
return connectionState(_that.state);case ReceiverUiEvent_VideoTrack() when videoTrack != null:
return videoTrack(_that.mime,_that.codec);case ReceiverUiEvent_Stats() when stats != null:
return stats(_that.packets,_that.bytes,_that.frames,_that.keyframes);case ReceiverUiEvent_FrameReady() when frameReady != null:
return frameReady();case ReceiverUiEvent_EncodedReady() when encodedReady != null:
return encodedReady();case ReceiverUiEvent_VideoStats() when videoStats != null:
return videoStats(_that.decoded,_that.delivered,_that.dropped,_that.droppedPackets,_that.decodeErrors,_that.keyframeRequests,_that.width,_that.height,_that.avgDecodeUs,_that.error);case ReceiverUiEvent_ContentTransferStarted() when contentTransferStarted != null:
return contentTransferStarted(_that.document,_that.name,_that.size);case ReceiverUiEvent_ContentTransferProgress() when contentTransferProgress != null:
return contentTransferProgress(_that.received,_that.total);case ReceiverUiEvent_ContentFrameReady() when contentFrameReady != null:
return contentFrameReady();case ReceiverUiEvent_ContentDocumentOpened() when contentDocumentOpened != null:
return contentDocumentOpened(_that.name,_that.pages);case ReceiverUiEvent_ContentLink() when contentLink != null:
return contentLink(_that.url,_that.host);case ReceiverUiEvent_ContentStopped() when contentStopped != null:
return contentStopped();case ReceiverUiEvent_ContentError() when contentError != null:
return contentError(_that.error);case ReceiverUiEvent_PeerLeft() when peerLeft != null:
return peerLeft();case ReceiverUiEvent_PresentationStopped() when presentationStopped != null:
return presentationStopped();case ReceiverUiEvent_PauseChanged() when pauseChanged != null:
return pauseChanged(_that.presenter,_that.display);case ReceiverUiEvent_ConnectionCode() when connectionCode != null:
return connectionCode(_that.sas);case ReceiverUiEvent_Error() when error != null:
return error(_that.code,_that.message);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String code,  UiJoinMode joinMode)  roomCode,required TResult Function( String name,  bool handover)  peerJoined,required TResult Function( String name)  handoverStored,required TResult Function()  handoverExpired,required TResult Function( BigInt id,  String name,  int check)  joinRequest,required TResult Function( BigInt id,  String reason)  joinRequestCancelled,required TResult Function( UiJoinMode mode)  joinModeChanged,required TResult Function( BigInt id,  String name)  blocked,required TResult Function( BigInt id)  unblocked,required TResult Function( String code,  String message)  serverNotice,required TResult Function()  roomEnded,required TResult Function( UiConnectionState state)  connectionState,required TResult Function( String mime,  String? codec)  videoTrack,required TResult Function( BigInt packets,  BigInt bytes,  BigInt frames,  BigInt keyframes)  stats,required TResult Function()  frameReady,required TResult Function()  encodedReady,required TResult Function( BigInt decoded,  BigInt delivered,  BigInt dropped,  BigInt droppedPackets,  BigInt decodeErrors,  BigInt keyframeRequests,  int width,  int height,  BigInt avgDecodeUs,  String? error)  videoStats,required TResult Function( bool document,  String name,  BigInt size)  contentTransferStarted,required TResult Function( BigInt received,  BigInt total)  contentTransferProgress,required TResult Function()  contentFrameReady,required TResult Function( String name,  int pages)  contentDocumentOpened,required TResult Function( String url,  String host)  contentLink,required TResult Function()  contentStopped,required TResult Function( UiContentError error)  contentError,required TResult Function()  peerLeft,required TResult Function()  presentationStopped,required TResult Function( bool presenter,  bool display)  pauseChanged,required TResult Function( String sas)  connectionCode,required TResult Function( String code,  String message)  error,}) {final _that = this;
switch (_that) {
case ReceiverUiEvent_RoomCode():
return roomCode(_that.code,_that.joinMode);case ReceiverUiEvent_PeerJoined():
return peerJoined(_that.name,_that.handover);case ReceiverUiEvent_HandoverStored():
return handoverStored(_that.name);case ReceiverUiEvent_HandoverExpired():
return handoverExpired();case ReceiverUiEvent_JoinRequest():
return joinRequest(_that.id,_that.name,_that.check);case ReceiverUiEvent_JoinRequestCancelled():
return joinRequestCancelled(_that.id,_that.reason);case ReceiverUiEvent_JoinModeChanged():
return joinModeChanged(_that.mode);case ReceiverUiEvent_Blocked():
return blocked(_that.id,_that.name);case ReceiverUiEvent_Unblocked():
return unblocked(_that.id);case ReceiverUiEvent_ServerNotice():
return serverNotice(_that.code,_that.message);case ReceiverUiEvent_RoomEnded():
return roomEnded();case ReceiverUiEvent_ConnectionState():
return connectionState(_that.state);case ReceiverUiEvent_VideoTrack():
return videoTrack(_that.mime,_that.codec);case ReceiverUiEvent_Stats():
return stats(_that.packets,_that.bytes,_that.frames,_that.keyframes);case ReceiverUiEvent_FrameReady():
return frameReady();case ReceiverUiEvent_EncodedReady():
return encodedReady();case ReceiverUiEvent_VideoStats():
return videoStats(_that.decoded,_that.delivered,_that.dropped,_that.droppedPackets,_that.decodeErrors,_that.keyframeRequests,_that.width,_that.height,_that.avgDecodeUs,_that.error);case ReceiverUiEvent_ContentTransferStarted():
return contentTransferStarted(_that.document,_that.name,_that.size);case ReceiverUiEvent_ContentTransferProgress():
return contentTransferProgress(_that.received,_that.total);case ReceiverUiEvent_ContentFrameReady():
return contentFrameReady();case ReceiverUiEvent_ContentDocumentOpened():
return contentDocumentOpened(_that.name,_that.pages);case ReceiverUiEvent_ContentLink():
return contentLink(_that.url,_that.host);case ReceiverUiEvent_ContentStopped():
return contentStopped();case ReceiverUiEvent_ContentError():
return contentError(_that.error);case ReceiverUiEvent_PeerLeft():
return peerLeft();case ReceiverUiEvent_PresentationStopped():
return presentationStopped();case ReceiverUiEvent_PauseChanged():
return pauseChanged(_that.presenter,_that.display);case ReceiverUiEvent_ConnectionCode():
return connectionCode(_that.sas);case ReceiverUiEvent_Error():
return error(_that.code,_that.message);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String code,  UiJoinMode joinMode)?  roomCode,TResult? Function( String name,  bool handover)?  peerJoined,TResult? Function( String name)?  handoverStored,TResult? Function()?  handoverExpired,TResult? Function( BigInt id,  String name,  int check)?  joinRequest,TResult? Function( BigInt id,  String reason)?  joinRequestCancelled,TResult? Function( UiJoinMode mode)?  joinModeChanged,TResult? Function( BigInt id,  String name)?  blocked,TResult? Function( BigInt id)?  unblocked,TResult? Function( String code,  String message)?  serverNotice,TResult? Function()?  roomEnded,TResult? Function( UiConnectionState state)?  connectionState,TResult? Function( String mime,  String? codec)?  videoTrack,TResult? Function( BigInt packets,  BigInt bytes,  BigInt frames,  BigInt keyframes)?  stats,TResult? Function()?  frameReady,TResult? Function()?  encodedReady,TResult? Function( BigInt decoded,  BigInt delivered,  BigInt dropped,  BigInt droppedPackets,  BigInt decodeErrors,  BigInt keyframeRequests,  int width,  int height,  BigInt avgDecodeUs,  String? error)?  videoStats,TResult? Function( bool document,  String name,  BigInt size)?  contentTransferStarted,TResult? Function( BigInt received,  BigInt total)?  contentTransferProgress,TResult? Function()?  contentFrameReady,TResult? Function( String name,  int pages)?  contentDocumentOpened,TResult? Function( String url,  String host)?  contentLink,TResult? Function()?  contentStopped,TResult? Function( UiContentError error)?  contentError,TResult? Function()?  peerLeft,TResult? Function()?  presentationStopped,TResult? Function( bool presenter,  bool display)?  pauseChanged,TResult? Function( String sas)?  connectionCode,TResult? Function( String code,  String message)?  error,}) {final _that = this;
switch (_that) {
case ReceiverUiEvent_RoomCode() when roomCode != null:
return roomCode(_that.code,_that.joinMode);case ReceiverUiEvent_PeerJoined() when peerJoined != null:
return peerJoined(_that.name,_that.handover);case ReceiverUiEvent_HandoverStored() when handoverStored != null:
return handoverStored(_that.name);case ReceiverUiEvent_HandoverExpired() when handoverExpired != null:
return handoverExpired();case ReceiverUiEvent_JoinRequest() when joinRequest != null:
return joinRequest(_that.id,_that.name,_that.check);case ReceiverUiEvent_JoinRequestCancelled() when joinRequestCancelled != null:
return joinRequestCancelled(_that.id,_that.reason);case ReceiverUiEvent_JoinModeChanged() when joinModeChanged != null:
return joinModeChanged(_that.mode);case ReceiverUiEvent_Blocked() when blocked != null:
return blocked(_that.id,_that.name);case ReceiverUiEvent_Unblocked() when unblocked != null:
return unblocked(_that.id);case ReceiverUiEvent_ServerNotice() when serverNotice != null:
return serverNotice(_that.code,_that.message);case ReceiverUiEvent_RoomEnded() when roomEnded != null:
return roomEnded();case ReceiverUiEvent_ConnectionState() when connectionState != null:
return connectionState(_that.state);case ReceiverUiEvent_VideoTrack() when videoTrack != null:
return videoTrack(_that.mime,_that.codec);case ReceiverUiEvent_Stats() when stats != null:
return stats(_that.packets,_that.bytes,_that.frames,_that.keyframes);case ReceiverUiEvent_FrameReady() when frameReady != null:
return frameReady();case ReceiverUiEvent_EncodedReady() when encodedReady != null:
return encodedReady();case ReceiverUiEvent_VideoStats() when videoStats != null:
return videoStats(_that.decoded,_that.delivered,_that.dropped,_that.droppedPackets,_that.decodeErrors,_that.keyframeRequests,_that.width,_that.height,_that.avgDecodeUs,_that.error);case ReceiverUiEvent_ContentTransferStarted() when contentTransferStarted != null:
return contentTransferStarted(_that.document,_that.name,_that.size);case ReceiverUiEvent_ContentTransferProgress() when contentTransferProgress != null:
return contentTransferProgress(_that.received,_that.total);case ReceiverUiEvent_ContentFrameReady() when contentFrameReady != null:
return contentFrameReady();case ReceiverUiEvent_ContentDocumentOpened() when contentDocumentOpened != null:
return contentDocumentOpened(_that.name,_that.pages);case ReceiverUiEvent_ContentLink() when contentLink != null:
return contentLink(_that.url,_that.host);case ReceiverUiEvent_ContentStopped() when contentStopped != null:
return contentStopped();case ReceiverUiEvent_ContentError() when contentError != null:
return contentError(_that.error);case ReceiverUiEvent_PeerLeft() when peerLeft != null:
return peerLeft();case ReceiverUiEvent_PresentationStopped() when presentationStopped != null:
return presentationStopped();case ReceiverUiEvent_PauseChanged() when pauseChanged != null:
return pauseChanged(_that.presenter,_that.display);case ReceiverUiEvent_ConnectionCode() when connectionCode != null:
return connectionCode(_that.sas);case ReceiverUiEvent_Error() when error != null:
return error(_that.code,_that.message);case _:
  return null;

}
}

}

/// @nodoc


class ReceiverUiEvent_RoomCode extends ReceiverUiEvent {
  const ReceiverUiEvent_RoomCode({required this.code, required this.joinMode}): super._();
  

/// The room code, uppercase.
 final  String code;
/// The join mode the room has.
 final  UiJoinMode joinMode;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiverUiEvent_RoomCodeCopyWith<ReceiverUiEvent_RoomCode> get copyWith => _$ReceiverUiEvent_RoomCodeCopyWithImpl<ReceiverUiEvent_RoomCode>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_RoomCode&&(identical(other.code, code) || other.code == code)&&(identical(other.joinMode, joinMode) || other.joinMode == joinMode));
}


@override
int get hashCode {
    return Object.hash(runtimeType,code,joinMode);
}

@override
String toString() {
    return 'ReceiverUiEvent.roomCode(code: $code, joinMode: $joinMode)';
}


}

/// @nodoc
abstract mixin class $ReceiverUiEvent_RoomCodeCopyWith<$Res> implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_RoomCodeCopyWith(ReceiverUiEvent_RoomCode value, $Res Function(ReceiverUiEvent_RoomCode) _then) = _$ReceiverUiEvent_RoomCodeCopyWithImpl;
@useResult
$Res call({
 String code, UiJoinMode joinMode
});




}
/// @nodoc
class _$ReceiverUiEvent_RoomCodeCopyWithImpl<$Res>
    implements $ReceiverUiEvent_RoomCodeCopyWith<$Res> {
  _$ReceiverUiEvent_RoomCodeCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_RoomCode _self;
  final $Res Function(ReceiverUiEvent_RoomCode) _then;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? code = null,Object? joinMode = null,}) {
  return _then(ReceiverUiEvent_RoomCode(
code: null == code ? _self.code : code // ignore: cast_nullable_to_non_nullable
as String,joinMode: null == joinMode ? _self.joinMode : joinMode // ignore: cast_nullable_to_non_nullable
as UiJoinMode,
  ));
}


}

/// @nodoc


class ReceiverUiEvent_PeerJoined extends ReceiverUiEvent {
  const ReceiverUiEvent_PeerJoined({required this.name, required this.handover}): super._();
  

/// Cleaned display name of the sender (plain text, at most 40 characters).
 final  String name;
/// The join came with a hand-over token this display had issued (the iOS broadcast
/// extension, native-apps.md 20.5) and was let in without a dialog: show no
/// "connected" notice, the person was approved before.
 final  bool handover;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiverUiEvent_PeerJoinedCopyWith<ReceiverUiEvent_PeerJoined> get copyWith => _$ReceiverUiEvent_PeerJoinedCopyWithImpl<ReceiverUiEvent_PeerJoined>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_PeerJoined&&(identical(other.name, name) || other.name == name)&&(identical(other.handover, handover) || other.handover == handover));
}


@override
int get hashCode {
    return Object.hash(runtimeType,name,handover);
}

@override
String toString() {
    return 'ReceiverUiEvent.peerJoined(name: $name, handover: $handover)';
}


}

/// @nodoc
abstract mixin class $ReceiverUiEvent_PeerJoinedCopyWith<$Res> implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_PeerJoinedCopyWith(ReceiverUiEvent_PeerJoined value, $Res Function(ReceiverUiEvent_PeerJoined) _then) = _$ReceiverUiEvent_PeerJoinedCopyWithImpl;
@useResult
$Res call({
 String name, bool handover
});




}
/// @nodoc
class _$ReceiverUiEvent_PeerJoinedCopyWithImpl<$Res>
    implements $ReceiverUiEvent_PeerJoinedCopyWith<$Res> {
  _$ReceiverUiEvent_PeerJoinedCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_PeerJoined _self;
  final $Res Function(ReceiverUiEvent_PeerJoined) _then;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? name = null,Object? handover = null,}) {
  return _then(ReceiverUiEvent_PeerJoined(
name: null == name ? _self.name : name // ignore: cast_nullable_to_non_nullable
as String,handover: null == handover ? _self.handover : handover // ignore: cast_nullable_to_non_nullable
as bool,
  ));
}


}

/// @nodoc


class ReceiverUiEvent_HandoverStored extends ReceiverUiEvent {
  const ReceiverUiEvent_HandoverStored({required this.name}): super._();
  

/// The presenter's name.
 final  String name;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiverUiEvent_HandoverStoredCopyWith<ReceiverUiEvent_HandoverStored> get copyWith => _$ReceiverUiEvent_HandoverStoredCopyWithImpl<ReceiverUiEvent_HandoverStored>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_HandoverStored&&(identical(other.name, name) || other.name == name));
}


@override
int get hashCode {
    return Object.hash(runtimeType,name);
}

@override
String toString() {
    return 'ReceiverUiEvent.handoverStored(name: $name)';
}


}

/// @nodoc
abstract mixin class $ReceiverUiEvent_HandoverStoredCopyWith<$Res> implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_HandoverStoredCopyWith(ReceiverUiEvent_HandoverStored value, $Res Function(ReceiverUiEvent_HandoverStored) _then) = _$ReceiverUiEvent_HandoverStoredCopyWithImpl;
@useResult
$Res call({
 String name
});




}
/// @nodoc
class _$ReceiverUiEvent_HandoverStoredCopyWithImpl<$Res>
    implements $ReceiverUiEvent_HandoverStoredCopyWith<$Res> {
  _$ReceiverUiEvent_HandoverStoredCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_HandoverStored _self;
  final $Res Function(ReceiverUiEvent_HandoverStored) _then;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? name = null,}) {
  return _then(ReceiverUiEvent_HandoverStored(
name: null == name ? _self.name : name // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ReceiverUiEvent_HandoverExpired extends ReceiverUiEvent {
  const ReceiverUiEvent_HandoverExpired(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_HandoverExpired);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ReceiverUiEvent.handoverExpired()';
}


}




/// @nodoc


class ReceiverUiEvent_JoinRequest extends ReceiverUiEvent {
  const ReceiverUiEvent_JoinRequest({required this.id, required this.name, required this.check}): super._();
  

/// Request id for [`decide_join`].
 final  BigInt id;
/// Cleaned display name.
 final  String name;
/// Two-digit check number; the sender's waiting screen shows the same number.
 final  int check;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiverUiEvent_JoinRequestCopyWith<ReceiverUiEvent_JoinRequest> get copyWith => _$ReceiverUiEvent_JoinRequestCopyWithImpl<ReceiverUiEvent_JoinRequest>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_JoinRequest&&(identical(other.id, id) || other.id == id)&&(identical(other.name, name) || other.name == name)&&(identical(other.check, check) || other.check == check));
}


@override
int get hashCode {
    return Object.hash(runtimeType,id,name,check);
}

@override
String toString() {
    return 'ReceiverUiEvent.joinRequest(id: $id, name: $name, check: $check)';
}


}

/// @nodoc
abstract mixin class $ReceiverUiEvent_JoinRequestCopyWith<$Res> implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_JoinRequestCopyWith(ReceiverUiEvent_JoinRequest value, $Res Function(ReceiverUiEvent_JoinRequest) _then) = _$ReceiverUiEvent_JoinRequestCopyWithImpl;
@useResult
$Res call({
 BigInt id, String name, int check
});




}
/// @nodoc
class _$ReceiverUiEvent_JoinRequestCopyWithImpl<$Res>
    implements $ReceiverUiEvent_JoinRequestCopyWith<$Res> {
  _$ReceiverUiEvent_JoinRequestCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_JoinRequest _self;
  final $Res Function(ReceiverUiEvent_JoinRequest) _then;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? id = null,Object? name = null,Object? check = null,}) {
  return _then(ReceiverUiEvent_JoinRequest(
id: null == id ? _self.id : id // ignore: cast_nullable_to_non_nullable
as BigInt,name: null == name ? _self.name : name // ignore: cast_nullable_to_non_nullable
as String,check: null == check ? _self.check : check // ignore: cast_nullable_to_non_nullable
as int,
  ));
}


}

/// @nodoc


class ReceiverUiEvent_JoinRequestCancelled extends ReceiverUiEvent {
  const ReceiverUiEvent_JoinRequestCancelled({required this.id, required this.reason}): super._();
  

/// Request id.
 final  BigInt id;
/// Why.
 final  String reason;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiverUiEvent_JoinRequestCancelledCopyWith<ReceiverUiEvent_JoinRequestCancelled> get copyWith => _$ReceiverUiEvent_JoinRequestCancelledCopyWithImpl<ReceiverUiEvent_JoinRequestCancelled>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_JoinRequestCancelled&&(identical(other.id, id) || other.id == id)&&(identical(other.reason, reason) || other.reason == reason));
}


@override
int get hashCode {
    return Object.hash(runtimeType,id,reason);
}

@override
String toString() {
    return 'ReceiverUiEvent.joinRequestCancelled(id: $id, reason: $reason)';
}


}

/// @nodoc
abstract mixin class $ReceiverUiEvent_JoinRequestCancelledCopyWith<$Res> implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_JoinRequestCancelledCopyWith(ReceiverUiEvent_JoinRequestCancelled value, $Res Function(ReceiverUiEvent_JoinRequestCancelled) _then) = _$ReceiverUiEvent_JoinRequestCancelledCopyWithImpl;
@useResult
$Res call({
 BigInt id, String reason
});




}
/// @nodoc
class _$ReceiverUiEvent_JoinRequestCancelledCopyWithImpl<$Res>
    implements $ReceiverUiEvent_JoinRequestCancelledCopyWith<$Res> {
  _$ReceiverUiEvent_JoinRequestCancelledCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_JoinRequestCancelled _self;
  final $Res Function(ReceiverUiEvent_JoinRequestCancelled) _then;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? id = null,Object? reason = null,}) {
  return _then(ReceiverUiEvent_JoinRequestCancelled(
id: null == id ? _self.id : id // ignore: cast_nullable_to_non_nullable
as BigInt,reason: null == reason ? _self.reason : reason // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ReceiverUiEvent_JoinModeChanged extends ReceiverUiEvent {
  const ReceiverUiEvent_JoinModeChanged({required this.mode}): super._();
  

/// The mode.
 final  UiJoinMode mode;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiverUiEvent_JoinModeChangedCopyWith<ReceiverUiEvent_JoinModeChanged> get copyWith => _$ReceiverUiEvent_JoinModeChangedCopyWithImpl<ReceiverUiEvent_JoinModeChanged>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_JoinModeChanged&&(identical(other.mode, mode) || other.mode == mode));
}


@override
int get hashCode {
    return Object.hash(runtimeType,mode);
}

@override
String toString() {
    return 'ReceiverUiEvent.joinModeChanged(mode: $mode)';
}


}

/// @nodoc
abstract mixin class $ReceiverUiEvent_JoinModeChangedCopyWith<$Res> implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_JoinModeChangedCopyWith(ReceiverUiEvent_JoinModeChanged value, $Res Function(ReceiverUiEvent_JoinModeChanged) _then) = _$ReceiverUiEvent_JoinModeChangedCopyWithImpl;
@useResult
$Res call({
 UiJoinMode mode
});




}
/// @nodoc
class _$ReceiverUiEvent_JoinModeChangedCopyWithImpl<$Res>
    implements $ReceiverUiEvent_JoinModeChangedCopyWith<$Res> {
  _$ReceiverUiEvent_JoinModeChangedCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_JoinModeChanged _self;
  final $Res Function(ReceiverUiEvent_JoinModeChanged) _then;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? mode = null,}) {
  return _then(ReceiverUiEvent_JoinModeChanged(
mode: null == mode ? _self.mode : mode // ignore: cast_nullable_to_non_nullable
as UiJoinMode,
  ));
}


}

/// @nodoc


class ReceiverUiEvent_Blocked extends ReceiverUiEvent {
  const ReceiverUiEvent_Blocked({required this.id, required this.name}): super._();
  

/// Block id for [`unblock`].
 final  BigInt id;
/// Cleaned display name (for the list only).
 final  String name;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiverUiEvent_BlockedCopyWith<ReceiverUiEvent_Blocked> get copyWith => _$ReceiverUiEvent_BlockedCopyWithImpl<ReceiverUiEvent_Blocked>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_Blocked&&(identical(other.id, id) || other.id == id)&&(identical(other.name, name) || other.name == name));
}


@override
int get hashCode {
    return Object.hash(runtimeType,id,name);
}

@override
String toString() {
    return 'ReceiverUiEvent.blocked(id: $id, name: $name)';
}


}

/// @nodoc
abstract mixin class $ReceiverUiEvent_BlockedCopyWith<$Res> implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_BlockedCopyWith(ReceiverUiEvent_Blocked value, $Res Function(ReceiverUiEvent_Blocked) _then) = _$ReceiverUiEvent_BlockedCopyWithImpl;
@useResult
$Res call({
 BigInt id, String name
});




}
/// @nodoc
class _$ReceiverUiEvent_BlockedCopyWithImpl<$Res>
    implements $ReceiverUiEvent_BlockedCopyWith<$Res> {
  _$ReceiverUiEvent_BlockedCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_Blocked _self;
  final $Res Function(ReceiverUiEvent_Blocked) _then;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? id = null,Object? name = null,}) {
  return _then(ReceiverUiEvent_Blocked(
id: null == id ? _self.id : id // ignore: cast_nullable_to_non_nullable
as BigInt,name: null == name ? _self.name : name // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ReceiverUiEvent_Unblocked extends ReceiverUiEvent {
  const ReceiverUiEvent_Unblocked({required this.id}): super._();
  

/// Block id.
 final  BigInt id;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiverUiEvent_UnblockedCopyWith<ReceiverUiEvent_Unblocked> get copyWith => _$ReceiverUiEvent_UnblockedCopyWithImpl<ReceiverUiEvent_Unblocked>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_Unblocked&&(identical(other.id, id) || other.id == id));
}


@override
int get hashCode {
    return Object.hash(runtimeType,id);
}

@override
String toString() {
    return 'ReceiverUiEvent.unblocked(id: $id)';
}


}

/// @nodoc
abstract mixin class $ReceiverUiEvent_UnblockedCopyWith<$Res> implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_UnblockedCopyWith(ReceiverUiEvent_Unblocked value, $Res Function(ReceiverUiEvent_Unblocked) _then) = _$ReceiverUiEvent_UnblockedCopyWithImpl;
@useResult
$Res call({
 BigInt id
});




}
/// @nodoc
class _$ReceiverUiEvent_UnblockedCopyWithImpl<$Res>
    implements $ReceiverUiEvent_UnblockedCopyWith<$Res> {
  _$ReceiverUiEvent_UnblockedCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_Unblocked _self;
  final $Res Function(ReceiverUiEvent_Unblocked) _then;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? id = null,}) {
  return _then(ReceiverUiEvent_Unblocked(
id: null == id ? _self.id : id // ignore: cast_nullable_to_non_nullable
as BigInt,
  ));
}


}

/// @nodoc


class ReceiverUiEvent_ServerNotice extends ReceiverUiEvent {
  const ReceiverUiEvent_ServerNotice({required this.code, required this.message}): super._();
  

/// Protocol error code (docs/protocol.md "Error codes").
 final  String code;
/// The server's English text, for logs.
 final  String message;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiverUiEvent_ServerNoticeCopyWith<ReceiverUiEvent_ServerNotice> get copyWith => _$ReceiverUiEvent_ServerNoticeCopyWithImpl<ReceiverUiEvent_ServerNotice>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_ServerNotice&&(identical(other.code, code) || other.code == code)&&(identical(other.message, message) || other.message == message));
}


@override
int get hashCode {
    return Object.hash(runtimeType,code,message);
}

@override
String toString() {
    return 'ReceiverUiEvent.serverNotice(code: $code, message: $message)';
}


}

/// @nodoc
abstract mixin class $ReceiverUiEvent_ServerNoticeCopyWith<$Res> implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_ServerNoticeCopyWith(ReceiverUiEvent_ServerNotice value, $Res Function(ReceiverUiEvent_ServerNotice) _then) = _$ReceiverUiEvent_ServerNoticeCopyWithImpl;
@useResult
$Res call({
 String code, String message
});




}
/// @nodoc
class _$ReceiverUiEvent_ServerNoticeCopyWithImpl<$Res>
    implements $ReceiverUiEvent_ServerNoticeCopyWith<$Res> {
  _$ReceiverUiEvent_ServerNoticeCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_ServerNotice _self;
  final $Res Function(ReceiverUiEvent_ServerNotice) _then;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? code = null,Object? message = null,}) {
  return _then(ReceiverUiEvent_ServerNotice(
code: null == code ? _self.code : code // ignore: cast_nullable_to_non_nullable
as String,message: null == message ? _self.message : message // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ReceiverUiEvent_RoomEnded extends ReceiverUiEvent {
  const ReceiverUiEvent_RoomEnded(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_RoomEnded);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ReceiverUiEvent.roomEnded()';
}


}




/// @nodoc


class ReceiverUiEvent_ConnectionState extends ReceiverUiEvent {
  const ReceiverUiEvent_ConnectionState({required this.state}): super._();
  

/// New state.
 final  UiConnectionState state;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiverUiEvent_ConnectionStateCopyWith<ReceiverUiEvent_ConnectionState> get copyWith => _$ReceiverUiEvent_ConnectionStateCopyWithImpl<ReceiverUiEvent_ConnectionState>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_ConnectionState&&(identical(other.state, state) || other.state == state));
}


@override
int get hashCode {
    return Object.hash(runtimeType,state);
}

@override
String toString() {
    return 'ReceiverUiEvent.connectionState(state: $state)';
}


}

/// @nodoc
abstract mixin class $ReceiverUiEvent_ConnectionStateCopyWith<$Res> implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_ConnectionStateCopyWith(ReceiverUiEvent_ConnectionState value, $Res Function(ReceiverUiEvent_ConnectionState) _then) = _$ReceiverUiEvent_ConnectionStateCopyWithImpl;
@useResult
$Res call({
 UiConnectionState state
});




}
/// @nodoc
class _$ReceiverUiEvent_ConnectionStateCopyWithImpl<$Res>
    implements $ReceiverUiEvent_ConnectionStateCopyWith<$Res> {
  _$ReceiverUiEvent_ConnectionStateCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_ConnectionState _self;
  final $Res Function(ReceiverUiEvent_ConnectionState) _then;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? state = null,}) {
  return _then(ReceiverUiEvent_ConnectionState(
state: null == state ? _self.state : state // ignore: cast_nullable_to_non_nullable
as UiConnectionState,
  ));
}


}

/// @nodoc


class ReceiverUiEvent_VideoTrack extends ReceiverUiEvent {
  const ReceiverUiEvent_VideoTrack({required this.mime, this.codec}): super._();
  

/// MIME type, e.g. `video/VP8`.
 final  String mime;
/// Codec name (`vp8`, `vp9`, `h264`, `av1`) or `None` if unknown.
 final  String? codec;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiverUiEvent_VideoTrackCopyWith<ReceiverUiEvent_VideoTrack> get copyWith => _$ReceiverUiEvent_VideoTrackCopyWithImpl<ReceiverUiEvent_VideoTrack>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_VideoTrack&&(identical(other.mime, mime) || other.mime == mime)&&(identical(other.codec, codec) || other.codec == codec));
}


@override
int get hashCode {
    return Object.hash(runtimeType,mime,codec);
}

@override
String toString() {
    return 'ReceiverUiEvent.videoTrack(mime: $mime, codec: $codec)';
}


}

/// @nodoc
abstract mixin class $ReceiverUiEvent_VideoTrackCopyWith<$Res> implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_VideoTrackCopyWith(ReceiverUiEvent_VideoTrack value, $Res Function(ReceiverUiEvent_VideoTrack) _then) = _$ReceiverUiEvent_VideoTrackCopyWithImpl;
@useResult
$Res call({
 String mime, String? codec
});




}
/// @nodoc
class _$ReceiverUiEvent_VideoTrackCopyWithImpl<$Res>
    implements $ReceiverUiEvent_VideoTrackCopyWith<$Res> {
  _$ReceiverUiEvent_VideoTrackCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_VideoTrack _self;
  final $Res Function(ReceiverUiEvent_VideoTrack) _then;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? mime = null,Object? codec = freezed,}) {
  return _then(ReceiverUiEvent_VideoTrack(
mime: null == mime ? _self.mime : mime // ignore: cast_nullable_to_non_nullable
as String,codec: freezed == codec ? _self.codec : codec // ignore: cast_nullable_to_non_nullable
as String?,
  ));
}


}

/// @nodoc


class ReceiverUiEvent_Stats extends ReceiverUiEvent {
  const ReceiverUiEvent_Stats({required this.packets, required this.bytes, required this.frames, required this.keyframes}): super._();
  

/// RTP packets received.
 final  BigInt packets;
/// RTP payload bytes received.
 final  BigInt bytes;
/// Frames received.
 final  BigInt frames;
/// Key frames received.
 final  BigInt keyframes;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiverUiEvent_StatsCopyWith<ReceiverUiEvent_Stats> get copyWith => _$ReceiverUiEvent_StatsCopyWithImpl<ReceiverUiEvent_Stats>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_Stats&&(identical(other.packets, packets) || other.packets == packets)&&(identical(other.bytes, bytes) || other.bytes == bytes)&&(identical(other.frames, frames) || other.frames == frames)&&(identical(other.keyframes, keyframes) || other.keyframes == keyframes));
}


@override
int get hashCode {
    return Object.hash(runtimeType,packets,bytes,frames,keyframes);
}

@override
String toString() {
    return 'ReceiverUiEvent.stats(packets: $packets, bytes: $bytes, frames: $frames, keyframes: $keyframes)';
}


}

/// @nodoc
abstract mixin class $ReceiverUiEvent_StatsCopyWith<$Res> implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_StatsCopyWith(ReceiverUiEvent_Stats value, $Res Function(ReceiverUiEvent_Stats) _then) = _$ReceiverUiEvent_StatsCopyWithImpl;
@useResult
$Res call({
 BigInt packets, BigInt bytes, BigInt frames, BigInt keyframes
});




}
/// @nodoc
class _$ReceiverUiEvent_StatsCopyWithImpl<$Res>
    implements $ReceiverUiEvent_StatsCopyWith<$Res> {
  _$ReceiverUiEvent_StatsCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_Stats _self;
  final $Res Function(ReceiverUiEvent_Stats) _then;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? packets = null,Object? bytes = null,Object? frames = null,Object? keyframes = null,}) {
  return _then(ReceiverUiEvent_Stats(
packets: null == packets ? _self.packets : packets // ignore: cast_nullable_to_non_nullable
as BigInt,bytes: null == bytes ? _self.bytes : bytes // ignore: cast_nullable_to_non_nullable
as BigInt,frames: null == frames ? _self.frames : frames // ignore: cast_nullable_to_non_nullable
as BigInt,keyframes: null == keyframes ? _self.keyframes : keyframes // ignore: cast_nullable_to_non_nullable
as BigInt,
  ));
}


}

/// @nodoc


class ReceiverUiEvent_FrameReady extends ReceiverUiEvent {
  const ReceiverUiEvent_FrameReady(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_FrameReady);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ReceiverUiEvent.frameReady()';
}


}




/// @nodoc


class ReceiverUiEvent_EncodedReady extends ReceiverUiEvent {
  const ReceiverUiEvent_EncodedReady(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_EncodedReady);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ReceiverUiEvent.encodedReady()';
}


}




/// @nodoc


class ReceiverUiEvent_VideoStats extends ReceiverUiEvent {
  const ReceiverUiEvent_VideoStats({required this.decoded, required this.delivered, required this.dropped, required this.droppedPackets, required this.decodeErrors, required this.keyframeRequests, required this.width, required this.height, required this.avgDecodeUs, this.error}): super._();
  

/// Pictures decoded.
 final  BigInt decoded;
/// Pictures handed to the UI by `take_frame`.
 final  BigInt delivered;
/// Pictures replaced before the UI fetched them (the UI was too slow).
 final  BigInt dropped;
/// RTP packets thrown away because the decoder fell behind.
 final  BigInt droppedPackets;
/// Decoder errors.
 final  BigInt decodeErrors;
/// Key frame requests (PLI) sent to the sender.
 final  BigInt keyframeRequests;
/// Size of the last picture handed out (after scaling).
 final  int width;
/// Size of the last picture handed out (after scaling).
 final  int height;
/// Mean decode + scale time per picture in microseconds.
 final  BigInt avgDecodeUs;
/// Last decoder problem in English, or `None`.
 final  String? error;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiverUiEvent_VideoStatsCopyWith<ReceiverUiEvent_VideoStats> get copyWith => _$ReceiverUiEvent_VideoStatsCopyWithImpl<ReceiverUiEvent_VideoStats>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_VideoStats&&(identical(other.decoded, decoded) || other.decoded == decoded)&&(identical(other.delivered, delivered) || other.delivered == delivered)&&(identical(other.dropped, dropped) || other.dropped == dropped)&&(identical(other.droppedPackets, droppedPackets) || other.droppedPackets == droppedPackets)&&(identical(other.decodeErrors, decodeErrors) || other.decodeErrors == decodeErrors)&&(identical(other.keyframeRequests, keyframeRequests) || other.keyframeRequests == keyframeRequests)&&(identical(other.width, width) || other.width == width)&&(identical(other.height, height) || other.height == height)&&(identical(other.avgDecodeUs, avgDecodeUs) || other.avgDecodeUs == avgDecodeUs)&&(identical(other.error, error) || other.error == error));
}


@override
int get hashCode {
    return Object.hash(runtimeType,decoded,delivered,dropped,droppedPackets,decodeErrors,keyframeRequests,width,height,avgDecodeUs,error);
}

@override
String toString() {
    return 'ReceiverUiEvent.videoStats(decoded: $decoded, delivered: $delivered, dropped: $dropped, droppedPackets: $droppedPackets, decodeErrors: $decodeErrors, keyframeRequests: $keyframeRequests, width: $width, height: $height, avgDecodeUs: $avgDecodeUs, error: $error)';
}


}

/// @nodoc
abstract mixin class $ReceiverUiEvent_VideoStatsCopyWith<$Res> implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_VideoStatsCopyWith(ReceiverUiEvent_VideoStats value, $Res Function(ReceiverUiEvent_VideoStats) _then) = _$ReceiverUiEvent_VideoStatsCopyWithImpl;
@useResult
$Res call({
 BigInt decoded, BigInt delivered, BigInt dropped, BigInt droppedPackets, BigInt decodeErrors, BigInt keyframeRequests, int width, int height, BigInt avgDecodeUs, String? error
});




}
/// @nodoc
class _$ReceiverUiEvent_VideoStatsCopyWithImpl<$Res>
    implements $ReceiverUiEvent_VideoStatsCopyWith<$Res> {
  _$ReceiverUiEvent_VideoStatsCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_VideoStats _self;
  final $Res Function(ReceiverUiEvent_VideoStats) _then;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? decoded = null,Object? delivered = null,Object? dropped = null,Object? droppedPackets = null,Object? decodeErrors = null,Object? keyframeRequests = null,Object? width = null,Object? height = null,Object? avgDecodeUs = null,Object? error = freezed,}) {
  return _then(ReceiverUiEvent_VideoStats(
decoded: null == decoded ? _self.decoded : decoded // ignore: cast_nullable_to_non_nullable
as BigInt,delivered: null == delivered ? _self.delivered : delivered // ignore: cast_nullable_to_non_nullable
as BigInt,dropped: null == dropped ? _self.dropped : dropped // ignore: cast_nullable_to_non_nullable
as BigInt,droppedPackets: null == droppedPackets ? _self.droppedPackets : droppedPackets // ignore: cast_nullable_to_non_nullable
as BigInt,decodeErrors: null == decodeErrors ? _self.decodeErrors : decodeErrors // ignore: cast_nullable_to_non_nullable
as BigInt,keyframeRequests: null == keyframeRequests ? _self.keyframeRequests : keyframeRequests // ignore: cast_nullable_to_non_nullable
as BigInt,width: null == width ? _self.width : width // ignore: cast_nullable_to_non_nullable
as int,height: null == height ? _self.height : height // ignore: cast_nullable_to_non_nullable
as int,avgDecodeUs: null == avgDecodeUs ? _self.avgDecodeUs : avgDecodeUs // ignore: cast_nullable_to_non_nullable
as BigInt,error: freezed == error ? _self.error : error // ignore: cast_nullable_to_non_nullable
as String?,
  ));
}


}

/// @nodoc


class ReceiverUiEvent_ContentTransferStarted extends ReceiverUiEvent {
  const ReceiverUiEvent_ContentTransferStarted({required this.document, required this.name, required this.size}): super._();
  

/// `true` for a PDF, `false` for an image.
 final  bool document;
/// File name, already cleaned of control and invisible characters.
 final  String name;
/// Announced size in bytes.
 final  BigInt size;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiverUiEvent_ContentTransferStartedCopyWith<ReceiverUiEvent_ContentTransferStarted> get copyWith => _$ReceiverUiEvent_ContentTransferStartedCopyWithImpl<ReceiverUiEvent_ContentTransferStarted>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_ContentTransferStarted&&(identical(other.document, document) || other.document == document)&&(identical(other.name, name) || other.name == name)&&(identical(other.size, size) || other.size == size));
}


@override
int get hashCode {
    return Object.hash(runtimeType,document,name,size);
}

@override
String toString() {
    return 'ReceiverUiEvent.contentTransferStarted(document: $document, name: $name, size: $size)';
}


}

/// @nodoc
abstract mixin class $ReceiverUiEvent_ContentTransferStartedCopyWith<$Res> implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_ContentTransferStartedCopyWith(ReceiverUiEvent_ContentTransferStarted value, $Res Function(ReceiverUiEvent_ContentTransferStarted) _then) = _$ReceiverUiEvent_ContentTransferStartedCopyWithImpl;
@useResult
$Res call({
 bool document, String name, BigInt size
});




}
/// @nodoc
class _$ReceiverUiEvent_ContentTransferStartedCopyWithImpl<$Res>
    implements $ReceiverUiEvent_ContentTransferStartedCopyWith<$Res> {
  _$ReceiverUiEvent_ContentTransferStartedCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_ContentTransferStarted _self;
  final $Res Function(ReceiverUiEvent_ContentTransferStarted) _then;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? document = null,Object? name = null,Object? size = null,}) {
  return _then(ReceiverUiEvent_ContentTransferStarted(
document: null == document ? _self.document : document // ignore: cast_nullable_to_non_nullable
as bool,name: null == name ? _self.name : name // ignore: cast_nullable_to_non_nullable
as String,size: null == size ? _self.size : size // ignore: cast_nullable_to_non_nullable
as BigInt,
  ));
}


}

/// @nodoc


class ReceiverUiEvent_ContentTransferProgress extends ReceiverUiEvent {
  const ReceiverUiEvent_ContentTransferProgress({required this.received, required this.total}): super._();
  

/// Bytes received.
 final  BigInt received;
/// Announced size in bytes.
 final  BigInt total;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiverUiEvent_ContentTransferProgressCopyWith<ReceiverUiEvent_ContentTransferProgress> get copyWith => _$ReceiverUiEvent_ContentTransferProgressCopyWithImpl<ReceiverUiEvent_ContentTransferProgress>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_ContentTransferProgress&&(identical(other.received, received) || other.received == received)&&(identical(other.total, total) || other.total == total));
}


@override
int get hashCode {
    return Object.hash(runtimeType,received,total);
}

@override
String toString() {
    return 'ReceiverUiEvent.contentTransferProgress(received: $received, total: $total)';
}


}

/// @nodoc
abstract mixin class $ReceiverUiEvent_ContentTransferProgressCopyWith<$Res> implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_ContentTransferProgressCopyWith(ReceiverUiEvent_ContentTransferProgress value, $Res Function(ReceiverUiEvent_ContentTransferProgress) _then) = _$ReceiverUiEvent_ContentTransferProgressCopyWithImpl;
@useResult
$Res call({
 BigInt received, BigInt total
});




}
/// @nodoc
class _$ReceiverUiEvent_ContentTransferProgressCopyWithImpl<$Res>
    implements $ReceiverUiEvent_ContentTransferProgressCopyWith<$Res> {
  _$ReceiverUiEvent_ContentTransferProgressCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_ContentTransferProgress _self;
  final $Res Function(ReceiverUiEvent_ContentTransferProgress) _then;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? received = null,Object? total = null,}) {
  return _then(ReceiverUiEvent_ContentTransferProgress(
received: null == received ? _self.received : received // ignore: cast_nullable_to_non_nullable
as BigInt,total: null == total ? _self.total : total // ignore: cast_nullable_to_non_nullable
as BigInt,
  ));
}


}

/// @nodoc


class ReceiverUiEvent_ContentFrameReady extends ReceiverUiEvent {
  const ReceiverUiEvent_ContentFrameReady(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_ContentFrameReady);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ReceiverUiEvent.contentFrameReady()';
}


}




/// @nodoc


class ReceiverUiEvent_ContentDocumentOpened extends ReceiverUiEvent {
  const ReceiverUiEvent_ContentDocumentOpened({required this.name, required this.pages}): super._();
  

/// File name, cleaned.
 final  String name;
/// Number of pages (at least 1).
 final  int pages;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiverUiEvent_ContentDocumentOpenedCopyWith<ReceiverUiEvent_ContentDocumentOpened> get copyWith => _$ReceiverUiEvent_ContentDocumentOpenedCopyWithImpl<ReceiverUiEvent_ContentDocumentOpened>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_ContentDocumentOpened&&(identical(other.name, name) || other.name == name)&&(identical(other.pages, pages) || other.pages == pages));
}


@override
int get hashCode {
    return Object.hash(runtimeType,name,pages);
}

@override
String toString() {
    return 'ReceiverUiEvent.contentDocumentOpened(name: $name, pages: $pages)';
}


}

/// @nodoc
abstract mixin class $ReceiverUiEvent_ContentDocumentOpenedCopyWith<$Res> implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_ContentDocumentOpenedCopyWith(ReceiverUiEvent_ContentDocumentOpened value, $Res Function(ReceiverUiEvent_ContentDocumentOpened) _then) = _$ReceiverUiEvent_ContentDocumentOpenedCopyWithImpl;
@useResult
$Res call({
 String name, int pages
});




}
/// @nodoc
class _$ReceiverUiEvent_ContentDocumentOpenedCopyWithImpl<$Res>
    implements $ReceiverUiEvent_ContentDocumentOpenedCopyWith<$Res> {
  _$ReceiverUiEvent_ContentDocumentOpenedCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_ContentDocumentOpened _self;
  final $Res Function(ReceiverUiEvent_ContentDocumentOpened) _then;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? name = null,Object? pages = null,}) {
  return _then(ReceiverUiEvent_ContentDocumentOpened(
name: null == name ? _self.name : name // ignore: cast_nullable_to_non_nullable
as String,pages: null == pages ? _self.pages : pages // ignore: cast_nullable_to_non_nullable
as int,
  ));
}


}

/// @nodoc


class ReceiverUiEvent_ContentLink extends ReceiverUiEvent {
  const ReceiverUiEvent_ContentLink({required this.url, required this.host}): super._();
  

/// The normalised URL (ASCII, http or https).
 final  String url;
/// Host in its punycode form (an internationalised name shows as `xn--...`).
 final  String host;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiverUiEvent_ContentLinkCopyWith<ReceiverUiEvent_ContentLink> get copyWith => _$ReceiverUiEvent_ContentLinkCopyWithImpl<ReceiverUiEvent_ContentLink>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_ContentLink&&(identical(other.url, url) || other.url == url)&&(identical(other.host, host) || other.host == host));
}


@override
int get hashCode {
    return Object.hash(runtimeType,url,host);
}

@override
String toString() {
    return 'ReceiverUiEvent.contentLink(url: $url, host: $host)';
}


}

/// @nodoc
abstract mixin class $ReceiverUiEvent_ContentLinkCopyWith<$Res> implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_ContentLinkCopyWith(ReceiverUiEvent_ContentLink value, $Res Function(ReceiverUiEvent_ContentLink) _then) = _$ReceiverUiEvent_ContentLinkCopyWithImpl;
@useResult
$Res call({
 String url, String host
});




}
/// @nodoc
class _$ReceiverUiEvent_ContentLinkCopyWithImpl<$Res>
    implements $ReceiverUiEvent_ContentLinkCopyWith<$Res> {
  _$ReceiverUiEvent_ContentLinkCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_ContentLink _self;
  final $Res Function(ReceiverUiEvent_ContentLink) _then;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? url = null,Object? host = null,}) {
  return _then(ReceiverUiEvent_ContentLink(
url: null == url ? _self.url : url // ignore: cast_nullable_to_non_nullable
as String,host: null == host ? _self.host : host // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ReceiverUiEvent_ContentStopped extends ReceiverUiEvent {
  const ReceiverUiEvent_ContentStopped(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_ContentStopped);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ReceiverUiEvent.contentStopped()';
}


}




/// @nodoc


class ReceiverUiEvent_ContentError extends ReceiverUiEvent {
  const ReceiverUiEvent_ContentError({required this.error}): super._();
  

/// What happened.
 final  UiContentError error;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiverUiEvent_ContentErrorCopyWith<ReceiverUiEvent_ContentError> get copyWith => _$ReceiverUiEvent_ContentErrorCopyWithImpl<ReceiverUiEvent_ContentError>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_ContentError&&(identical(other.error, error) || other.error == error));
}


@override
int get hashCode {
    return Object.hash(runtimeType,error);
}

@override
String toString() {
    return 'ReceiverUiEvent.contentError(error: $error)';
}


}

/// @nodoc
abstract mixin class $ReceiverUiEvent_ContentErrorCopyWith<$Res> implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_ContentErrorCopyWith(ReceiverUiEvent_ContentError value, $Res Function(ReceiverUiEvent_ContentError) _then) = _$ReceiverUiEvent_ContentErrorCopyWithImpl;
@useResult
$Res call({
 UiContentError error
});




}
/// @nodoc
class _$ReceiverUiEvent_ContentErrorCopyWithImpl<$Res>
    implements $ReceiverUiEvent_ContentErrorCopyWith<$Res> {
  _$ReceiverUiEvent_ContentErrorCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_ContentError _self;
  final $Res Function(ReceiverUiEvent_ContentError) _then;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? error = null,}) {
  return _then(ReceiverUiEvent_ContentError(
error: null == error ? _self.error : error // ignore: cast_nullable_to_non_nullable
as UiContentError,
  ));
}


}

/// @nodoc


class ReceiverUiEvent_PeerLeft extends ReceiverUiEvent {
  const ReceiverUiEvent_PeerLeft(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_PeerLeft);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ReceiverUiEvent.peerLeft()';
}


}




/// @nodoc


class ReceiverUiEvent_PresentationStopped extends ReceiverUiEvent {
  const ReceiverUiEvent_PresentationStopped(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_PresentationStopped);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ReceiverUiEvent.presentationStopped()';
}


}




/// @nodoc


class ReceiverUiEvent_PauseChanged extends ReceiverUiEvent {
  const ReceiverUiEvent_PauseChanged({required this.presenter, required this.display}): super._();
  

/// The presenter paused its screen (it stopped sending video).
 final  bool presenter;
/// The display's own flag ([`set_display_paused`]).
 final  bool display;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiverUiEvent_PauseChangedCopyWith<ReceiverUiEvent_PauseChanged> get copyWith => _$ReceiverUiEvent_PauseChangedCopyWithImpl<ReceiverUiEvent_PauseChanged>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_PauseChanged&&(identical(other.presenter, presenter) || other.presenter == presenter)&&(identical(other.display, display) || other.display == display));
}


@override
int get hashCode {
    return Object.hash(runtimeType,presenter,display);
}

@override
String toString() {
    return 'ReceiverUiEvent.pauseChanged(presenter: $presenter, display: $display)';
}


}

/// @nodoc
abstract mixin class $ReceiverUiEvent_PauseChangedCopyWith<$Res> implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_PauseChangedCopyWith(ReceiverUiEvent_PauseChanged value, $Res Function(ReceiverUiEvent_PauseChanged) _then) = _$ReceiverUiEvent_PauseChangedCopyWithImpl;
@useResult
$Res call({
 bool presenter, bool display
});




}
/// @nodoc
class _$ReceiverUiEvent_PauseChangedCopyWithImpl<$Res>
    implements $ReceiverUiEvent_PauseChangedCopyWith<$Res> {
  _$ReceiverUiEvent_PauseChangedCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_PauseChanged _self;
  final $Res Function(ReceiverUiEvent_PauseChanged) _then;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? presenter = null,Object? display = null,}) {
  return _then(ReceiverUiEvent_PauseChanged(
presenter: null == presenter ? _self.presenter : presenter // ignore: cast_nullable_to_non_nullable
as bool,display: null == display ? _self.display : display // ignore: cast_nullable_to_non_nullable
as bool,
  ));
}


}

/// @nodoc


class ReceiverUiEvent_ConnectionCode extends ReceiverUiEvent {
  const ReceiverUiEvent_ConnectionCode({required this.sas}): super._();
  

/// The six digits with a space in the middle.
 final  String sas;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiverUiEvent_ConnectionCodeCopyWith<ReceiverUiEvent_ConnectionCode> get copyWith => _$ReceiverUiEvent_ConnectionCodeCopyWithImpl<ReceiverUiEvent_ConnectionCode>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_ConnectionCode&&(identical(other.sas, sas) || other.sas == sas));
}


@override
int get hashCode {
    return Object.hash(runtimeType,sas);
}

@override
String toString() {
    return 'ReceiverUiEvent.connectionCode(sas: $sas)';
}


}

/// @nodoc
abstract mixin class $ReceiverUiEvent_ConnectionCodeCopyWith<$Res> implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_ConnectionCodeCopyWith(ReceiverUiEvent_ConnectionCode value, $Res Function(ReceiverUiEvent_ConnectionCode) _then) = _$ReceiverUiEvent_ConnectionCodeCopyWithImpl;
@useResult
$Res call({
 String sas
});




}
/// @nodoc
class _$ReceiverUiEvent_ConnectionCodeCopyWithImpl<$Res>
    implements $ReceiverUiEvent_ConnectionCodeCopyWith<$Res> {
  _$ReceiverUiEvent_ConnectionCodeCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_ConnectionCode _self;
  final $Res Function(ReceiverUiEvent_ConnectionCode) _then;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? sas = null,}) {
  return _then(ReceiverUiEvent_ConnectionCode(
sas: null == sas ? _self.sas : sas // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ReceiverUiEvent_Error extends ReceiverUiEvent {
  const ReceiverUiEvent_Error({required this.code, required this.message}): super._();
  

/// The code the UI maps: a protocol code (`room_locked`, `server_full`, ...) or
/// `connection_lost`, `peer_failed`, `unknown`.
 final  String code;
/// Technical message in English (for logs only).
 final  String message;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiverUiEvent_ErrorCopyWith<ReceiverUiEvent_Error> get copyWith => _$ReceiverUiEvent_ErrorCopyWithImpl<ReceiverUiEvent_Error>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_Error&&(identical(other.code, code) || other.code == code)&&(identical(other.message, message) || other.message == message));
}


@override
int get hashCode {
    return Object.hash(runtimeType,code,message);
}

@override
String toString() {
    return 'ReceiverUiEvent.error(code: $code, message: $message)';
}


}

/// @nodoc
abstract mixin class $ReceiverUiEvent_ErrorCopyWith<$Res> implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_ErrorCopyWith(ReceiverUiEvent_Error value, $Res Function(ReceiverUiEvent_Error) _then) = _$ReceiverUiEvent_ErrorCopyWithImpl;
@useResult
$Res call({
 String code, String message
});




}
/// @nodoc
class _$ReceiverUiEvent_ErrorCopyWithImpl<$Res>
    implements $ReceiverUiEvent_ErrorCopyWith<$Res> {
  _$ReceiverUiEvent_ErrorCopyWithImpl(this._self, this._then);

  final ReceiverUiEvent_Error _self;
  final $Res Function(ReceiverUiEvent_Error) _then;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? code = null,Object? message = null,}) {
  return _then(ReceiverUiEvent_Error(
code: null == code ? _self.code : code // ignore: cast_nullable_to_non_nullable
as String,message: null == message ? _self.message : message // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc
mixin _$RoomCodeCheck {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is RoomCodeCheck);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'RoomCodeCheck()';
}


}

/// @nodoc
class $RoomCodeCheckCopyWith<$Res>  {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( RoomCodeCheck_Valid value)?  valid,TResult Function( RoomCodeCheck_WrongLength value)?  wrongLength,TResult Function( RoomCodeCheck_InvalidChar value)?  invalidChar,required TResult orElse(),}){
final _that = this;
switch (_that) {
case RoomCodeCheck_Valid() when valid != null:
return valid(_that);case RoomCodeCheck_WrongLength() when wrongLength != null:
return wrongLength(_that);case RoomCodeCheck_InvalidChar() when invalidChar != null:
return invalidChar(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( RoomCodeCheck_Valid value)  valid,required TResult Function( RoomCodeCheck_WrongLength value)  wrongLength,required TResult Function( RoomCodeCheck_InvalidChar value)  invalidChar,}){
final _that = this;
switch (_that) {
case RoomCodeCheck_Valid():
return valid(_that);case RoomCodeCheck_WrongLength():
return wrongLength(_that);case RoomCodeCheck_InvalidChar():
return invalidChar(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( RoomCodeCheck_Valid value)?  valid,TResult? Function( RoomCodeCheck_WrongLength value)?  wrongLength,TResult? Function( RoomCodeCheck_InvalidChar value)?  invalidChar,}){
final _that = this;
switch (_that) {
case RoomCodeCheck_Valid() when valid != null:
return valid(_that);case RoomCodeCheck_WrongLength() when wrongLength != null:
return wrongLength(_that);case RoomCodeCheck_InvalidChar() when invalidChar != null:
return invalidChar(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String code)?  valid,TResult Function()?  wrongLength,TResult Function( String ch)?  invalidChar,required TResult orElse(),}) {final _that = this;
switch (_that) {
case RoomCodeCheck_Valid() when valid != null:
return valid(_that.code);case RoomCodeCheck_WrongLength() when wrongLength != null:
return wrongLength();case RoomCodeCheck_InvalidChar() when invalidChar != null:
return invalidChar(_that.ch);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String code)  valid,required TResult Function()  wrongLength,required TResult Function( String ch)  invalidChar,}) {final _that = this;
switch (_that) {
case RoomCodeCheck_Valid():
return valid(_that.code);case RoomCodeCheck_WrongLength():
return wrongLength();case RoomCodeCheck_InvalidChar():
return invalidChar(_that.ch);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String code)?  valid,TResult? Function()?  wrongLength,TResult? Function( String ch)?  invalidChar,}) {final _that = this;
switch (_that) {
case RoomCodeCheck_Valid() when valid != null:
return valid(_that.code);case RoomCodeCheck_WrongLength() when wrongLength != null:
return wrongLength();case RoomCodeCheck_InvalidChar() when invalidChar != null:
return invalidChar(_that.ch);case _:
  return null;

}
}

}

/// @nodoc


class RoomCodeCheck_Valid extends RoomCodeCheck {
  const RoomCodeCheck_Valid({required this.code}): super._();
  

/// The normalised room code.
 final  String code;

/// Create a copy of RoomCodeCheck
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$RoomCodeCheck_ValidCopyWith<RoomCodeCheck_Valid> get copyWith => _$RoomCodeCheck_ValidCopyWithImpl<RoomCodeCheck_Valid>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is RoomCodeCheck_Valid&&(identical(other.code, code) || other.code == code));
}


@override
int get hashCode {
    return Object.hash(runtimeType,code);
}

@override
String toString() {
    return 'RoomCodeCheck.valid(code: $code)';
}


}

/// @nodoc
abstract mixin class $RoomCodeCheck_ValidCopyWith<$Res> implements $RoomCodeCheckCopyWith<$Res> {
  factory $RoomCodeCheck_ValidCopyWith(RoomCodeCheck_Valid value, $Res Function(RoomCodeCheck_Valid) _then) = _$RoomCodeCheck_ValidCopyWithImpl;
@useResult
$Res call({
 String code
});




}
/// @nodoc
class _$RoomCodeCheck_ValidCopyWithImpl<$Res>
    implements $RoomCodeCheck_ValidCopyWith<$Res> {
  _$RoomCodeCheck_ValidCopyWithImpl(this._self, this._then);

  final RoomCodeCheck_Valid _self;
  final $Res Function(RoomCodeCheck_Valid) _then;

/// Create a copy of RoomCodeCheck
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? code = null,}) {
  return _then(RoomCodeCheck_Valid(
code: null == code ? _self.code : code // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class RoomCodeCheck_WrongLength extends RoomCodeCheck {
  const RoomCodeCheck_WrongLength(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is RoomCodeCheck_WrongLength);
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
  const RoomCodeCheck_InvalidChar({required this.ch}): super._();
  

/// The first offending character.
 final  String ch;

/// Create a copy of RoomCodeCheck
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$RoomCodeCheck_InvalidCharCopyWith<RoomCodeCheck_InvalidChar> get copyWith => _$RoomCodeCheck_InvalidCharCopyWithImpl<RoomCodeCheck_InvalidChar>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is RoomCodeCheck_InvalidChar&&(identical(other.ch, ch) || other.ch == ch));
}


@override
int get hashCode {
    return Object.hash(runtimeType,ch);
}

@override
String toString() {
    return 'RoomCodeCheck.invalidChar(ch: $ch)';
}


}

/// @nodoc
abstract mixin class $RoomCodeCheck_InvalidCharCopyWith<$Res> implements $RoomCodeCheckCopyWith<$Res> {
  factory $RoomCodeCheck_InvalidCharCopyWith(RoomCodeCheck_InvalidChar value, $Res Function(RoomCodeCheck_InvalidChar) _then) = _$RoomCodeCheck_InvalidCharCopyWithImpl;
@useResult
$Res call({
 String ch
});




}
/// @nodoc
class _$RoomCodeCheck_InvalidCharCopyWithImpl<$Res>
    implements $RoomCodeCheck_InvalidCharCopyWith<$Res> {
  _$RoomCodeCheck_InvalidCharCopyWithImpl(this._self, this._then);

  final RoomCodeCheck_InvalidChar _self;
  final $Res Function(RoomCodeCheck_InvalidChar) _then;

/// Create a copy of RoomCodeCheck
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? ch = null,}) {
  return _then(RoomCodeCheck_InvalidChar(
ch: null == ch ? _self.ch : ch // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc
mixin _$SenderUiEvent {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is SenderUiEvent);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'SenderUiEvent()';
}


}

/// @nodoc
class $SenderUiEventCopyWith<$Res>  {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( SenderUiEvent_WaitingForApproval value)?  waitingForApproval,TResult Function( SenderUiEvent_JoinRefused value)?  joinRefused,TResult Function( SenderUiEvent_RemovedByHost value)?  removedByHost,TResult Function( SenderUiEvent_Joined value)?  joined,TResult Function( SenderUiEvent_ConnectionState value)?  connectionState,TResult Function( SenderUiEvent_Streaming value)?  streaming,TResult Function( SenderUiEvent_EncodedStats value)?  encodedStats,TResult Function( SenderUiEvent_ScreenStats value)?  screenStats,TResult Function( SenderUiEvent_Finished value)?  finished,TResult Function( SenderUiEvent_PeerLeft value)?  peerLeft,TResult Function( SenderUiEvent_Error value)?  error,required TResult orElse(),}){
final _that = this;
switch (_that) {
case SenderUiEvent_WaitingForApproval() when waitingForApproval != null:
return waitingForApproval(_that);case SenderUiEvent_JoinRefused() when joinRefused != null:
return joinRefused(_that);case SenderUiEvent_RemovedByHost() when removedByHost != null:
return removedByHost(_that);case SenderUiEvent_Joined() when joined != null:
return joined(_that);case SenderUiEvent_ConnectionState() when connectionState != null:
return connectionState(_that);case SenderUiEvent_Streaming() when streaming != null:
return streaming(_that);case SenderUiEvent_EncodedStats() when encodedStats != null:
return encodedStats(_that);case SenderUiEvent_ScreenStats() when screenStats != null:
return screenStats(_that);case SenderUiEvent_Finished() when finished != null:
return finished(_that);case SenderUiEvent_PeerLeft() when peerLeft != null:
return peerLeft(_that);case SenderUiEvent_Error() when error != null:
return error(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( SenderUiEvent_WaitingForApproval value)  waitingForApproval,required TResult Function( SenderUiEvent_JoinRefused value)  joinRefused,required TResult Function( SenderUiEvent_RemovedByHost value)  removedByHost,required TResult Function( SenderUiEvent_Joined value)  joined,required TResult Function( SenderUiEvent_ConnectionState value)  connectionState,required TResult Function( SenderUiEvent_Streaming value)  streaming,required TResult Function( SenderUiEvent_EncodedStats value)  encodedStats,required TResult Function( SenderUiEvent_ScreenStats value)  screenStats,required TResult Function( SenderUiEvent_Finished value)  finished,required TResult Function( SenderUiEvent_PeerLeft value)  peerLeft,required TResult Function( SenderUiEvent_Error value)  error,}){
final _that = this;
switch (_that) {
case SenderUiEvent_WaitingForApproval():
return waitingForApproval(_that);case SenderUiEvent_JoinRefused():
return joinRefused(_that);case SenderUiEvent_RemovedByHost():
return removedByHost(_that);case SenderUiEvent_Joined():
return joined(_that);case SenderUiEvent_ConnectionState():
return connectionState(_that);case SenderUiEvent_Streaming():
return streaming(_that);case SenderUiEvent_EncodedStats():
return encodedStats(_that);case SenderUiEvent_ScreenStats():
return screenStats(_that);case SenderUiEvent_Finished():
return finished(_that);case SenderUiEvent_PeerLeft():
return peerLeft(_that);case SenderUiEvent_Error():
return error(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( SenderUiEvent_WaitingForApproval value)?  waitingForApproval,TResult? Function( SenderUiEvent_JoinRefused value)?  joinRefused,TResult? Function( SenderUiEvent_RemovedByHost value)?  removedByHost,TResult? Function( SenderUiEvent_Joined value)?  joined,TResult? Function( SenderUiEvent_ConnectionState value)?  connectionState,TResult? Function( SenderUiEvent_Streaming value)?  streaming,TResult? Function( SenderUiEvent_EncodedStats value)?  encodedStats,TResult? Function( SenderUiEvent_ScreenStats value)?  screenStats,TResult? Function( SenderUiEvent_Finished value)?  finished,TResult? Function( SenderUiEvent_PeerLeft value)?  peerLeft,TResult? Function( SenderUiEvent_Error value)?  error,}){
final _that = this;
switch (_that) {
case SenderUiEvent_WaitingForApproval() when waitingForApproval != null:
return waitingForApproval(_that);case SenderUiEvent_JoinRefused() when joinRefused != null:
return joinRefused(_that);case SenderUiEvent_RemovedByHost() when removedByHost != null:
return removedByHost(_that);case SenderUiEvent_Joined() when joined != null:
return joined(_that);case SenderUiEvent_ConnectionState() when connectionState != null:
return connectionState(_that);case SenderUiEvent_Streaming() when streaming != null:
return streaming(_that);case SenderUiEvent_EncodedStats() when encodedStats != null:
return encodedStats(_that);case SenderUiEvent_ScreenStats() when screenStats != null:
return screenStats(_that);case SenderUiEvent_Finished() when finished != null:
return finished(_that);case SenderUiEvent_PeerLeft() when peerLeft != null:
return peerLeft(_that);case SenderUiEvent_Error() when error != null:
return error(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( int timeoutSecs,  int check)?  waitingForApproval,TResult Function( String code)?  joinRefused,TResult Function()?  removedByHost,TResult Function()?  joined,TResult Function( UiConnectionState state)?  connectionState,TResult Function( BigInt framesSent)?  streaming,TResult Function( BigInt pushed,  BigInt sent,  BigInt dropped,  BigInt keyframes,  BigInt keyframeRequests,  BigInt bytes)?  encodedStats,TResult Function( BigInt polls,  BigInt changed,  BigInt encoded,  BigInt keyframes,  BigInt encodedBytes)?  screenStats,TResult Function( BigInt framesSent)?  finished,TResult Function()?  peerLeft,TResult Function( String message)?  error,required TResult orElse(),}) {final _that = this;
switch (_that) {
case SenderUiEvent_WaitingForApproval() when waitingForApproval != null:
return waitingForApproval(_that.timeoutSecs,_that.check);case SenderUiEvent_JoinRefused() when joinRefused != null:
return joinRefused(_that.code);case SenderUiEvent_RemovedByHost() when removedByHost != null:
return removedByHost();case SenderUiEvent_Joined() when joined != null:
return joined();case SenderUiEvent_ConnectionState() when connectionState != null:
return connectionState(_that.state);case SenderUiEvent_Streaming() when streaming != null:
return streaming(_that.framesSent);case SenderUiEvent_EncodedStats() when encodedStats != null:
return encodedStats(_that.pushed,_that.sent,_that.dropped,_that.keyframes,_that.keyframeRequests,_that.bytes);case SenderUiEvent_ScreenStats() when screenStats != null:
return screenStats(_that.polls,_that.changed,_that.encoded,_that.keyframes,_that.encodedBytes);case SenderUiEvent_Finished() when finished != null:
return finished(_that.framesSent);case SenderUiEvent_PeerLeft() when peerLeft != null:
return peerLeft();case SenderUiEvent_Error() when error != null:
return error(_that.message);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( int timeoutSecs,  int check)  waitingForApproval,required TResult Function( String code)  joinRefused,required TResult Function()  removedByHost,required TResult Function()  joined,required TResult Function( UiConnectionState state)  connectionState,required TResult Function( BigInt framesSent)  streaming,required TResult Function( BigInt pushed,  BigInt sent,  BigInt dropped,  BigInt keyframes,  BigInt keyframeRequests,  BigInt bytes)  encodedStats,required TResult Function( BigInt polls,  BigInt changed,  BigInt encoded,  BigInt keyframes,  BigInt encodedBytes)  screenStats,required TResult Function( BigInt framesSent)  finished,required TResult Function()  peerLeft,required TResult Function( String message)  error,}) {final _that = this;
switch (_that) {
case SenderUiEvent_WaitingForApproval():
return waitingForApproval(_that.timeoutSecs,_that.check);case SenderUiEvent_JoinRefused():
return joinRefused(_that.code);case SenderUiEvent_RemovedByHost():
return removedByHost();case SenderUiEvent_Joined():
return joined();case SenderUiEvent_ConnectionState():
return connectionState(_that.state);case SenderUiEvent_Streaming():
return streaming(_that.framesSent);case SenderUiEvent_EncodedStats():
return encodedStats(_that.pushed,_that.sent,_that.dropped,_that.keyframes,_that.keyframeRequests,_that.bytes);case SenderUiEvent_ScreenStats():
return screenStats(_that.polls,_that.changed,_that.encoded,_that.keyframes,_that.encodedBytes);case SenderUiEvent_Finished():
return finished(_that.framesSent);case SenderUiEvent_PeerLeft():
return peerLeft();case SenderUiEvent_Error():
return error(_that.message);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( int timeoutSecs,  int check)?  waitingForApproval,TResult? Function( String code)?  joinRefused,TResult? Function()?  removedByHost,TResult? Function()?  joined,TResult? Function( UiConnectionState state)?  connectionState,TResult? Function( BigInt framesSent)?  streaming,TResult? Function( BigInt pushed,  BigInt sent,  BigInt dropped,  BigInt keyframes,  BigInt keyframeRequests,  BigInt bytes)?  encodedStats,TResult? Function( BigInt polls,  BigInt changed,  BigInt encoded,  BigInt keyframes,  BigInt encodedBytes)?  screenStats,TResult? Function( BigInt framesSent)?  finished,TResult? Function()?  peerLeft,TResult? Function( String message)?  error,}) {final _that = this;
switch (_that) {
case SenderUiEvent_WaitingForApproval() when waitingForApproval != null:
return waitingForApproval(_that.timeoutSecs,_that.check);case SenderUiEvent_JoinRefused() when joinRefused != null:
return joinRefused(_that.code);case SenderUiEvent_RemovedByHost() when removedByHost != null:
return removedByHost();case SenderUiEvent_Joined() when joined != null:
return joined();case SenderUiEvent_ConnectionState() when connectionState != null:
return connectionState(_that.state);case SenderUiEvent_Streaming() when streaming != null:
return streaming(_that.framesSent);case SenderUiEvent_EncodedStats() when encodedStats != null:
return encodedStats(_that.pushed,_that.sent,_that.dropped,_that.keyframes,_that.keyframeRequests,_that.bytes);case SenderUiEvent_ScreenStats() when screenStats != null:
return screenStats(_that.polls,_that.changed,_that.encoded,_that.keyframes,_that.encodedBytes);case SenderUiEvent_Finished() when finished != null:
return finished(_that.framesSent);case SenderUiEvent_PeerLeft() when peerLeft != null:
return peerLeft();case SenderUiEvent_Error() when error != null:
return error(_that.message);case _:
  return null;

}
}

}

/// @nodoc


class SenderUiEvent_WaitingForApproval extends SenderUiEvent {
  const SenderUiEvent_WaitingForApproval({required this.timeoutSecs, required this.check}): super._();
  

/// Seconds until the server gives up.
 final  int timeoutSecs;
/// Two-digit check number.
 final  int check;

/// Create a copy of SenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SenderUiEvent_WaitingForApprovalCopyWith<SenderUiEvent_WaitingForApproval> get copyWith => _$SenderUiEvent_WaitingForApprovalCopyWithImpl<SenderUiEvent_WaitingForApproval>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is SenderUiEvent_WaitingForApproval&&(identical(other.timeoutSecs, timeoutSecs) || other.timeoutSecs == timeoutSecs)&&(identical(other.check, check) || other.check == check));
}


@override
int get hashCode {
    return Object.hash(runtimeType,timeoutSecs,check);
}

@override
String toString() {
    return 'SenderUiEvent.waitingForApproval(timeoutSecs: $timeoutSecs, check: $check)';
}


}

/// @nodoc
abstract mixin class $SenderUiEvent_WaitingForApprovalCopyWith<$Res> implements $SenderUiEventCopyWith<$Res> {
  factory $SenderUiEvent_WaitingForApprovalCopyWith(SenderUiEvent_WaitingForApproval value, $Res Function(SenderUiEvent_WaitingForApproval) _then) = _$SenderUiEvent_WaitingForApprovalCopyWithImpl;
@useResult
$Res call({
 int timeoutSecs, int check
});




}
/// @nodoc
class _$SenderUiEvent_WaitingForApprovalCopyWithImpl<$Res>
    implements $SenderUiEvent_WaitingForApprovalCopyWith<$Res> {
  _$SenderUiEvent_WaitingForApprovalCopyWithImpl(this._self, this._then);

  final SenderUiEvent_WaitingForApproval _self;
  final $Res Function(SenderUiEvent_WaitingForApproval) _then;

/// Create a copy of SenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? timeoutSecs = null,Object? check = null,}) {
  return _then(SenderUiEvent_WaitingForApproval(
timeoutSecs: null == timeoutSecs ? _self.timeoutSecs : timeoutSecs // ignore: cast_nullable_to_non_nullable
as int,check: null == check ? _self.check : check // ignore: cast_nullable_to_non_nullable
as int,
  ));
}


}

/// @nodoc


class SenderUiEvent_JoinRefused extends SenderUiEvent {
  const SenderUiEvent_JoinRefused({required this.code}): super._();
  

/// Server code.
 final  String code;

/// Create a copy of SenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SenderUiEvent_JoinRefusedCopyWith<SenderUiEvent_JoinRefused> get copyWith => _$SenderUiEvent_JoinRefusedCopyWithImpl<SenderUiEvent_JoinRefused>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is SenderUiEvent_JoinRefused&&(identical(other.code, code) || other.code == code));
}


@override
int get hashCode {
    return Object.hash(runtimeType,code);
}

@override
String toString() {
    return 'SenderUiEvent.joinRefused(code: $code)';
}


}

/// @nodoc
abstract mixin class $SenderUiEvent_JoinRefusedCopyWith<$Res> implements $SenderUiEventCopyWith<$Res> {
  factory $SenderUiEvent_JoinRefusedCopyWith(SenderUiEvent_JoinRefused value, $Res Function(SenderUiEvent_JoinRefused) _then) = _$SenderUiEvent_JoinRefusedCopyWithImpl;
@useResult
$Res call({
 String code
});




}
/// @nodoc
class _$SenderUiEvent_JoinRefusedCopyWithImpl<$Res>
    implements $SenderUiEvent_JoinRefusedCopyWith<$Res> {
  _$SenderUiEvent_JoinRefusedCopyWithImpl(this._self, this._then);

  final SenderUiEvent_JoinRefused _self;
  final $Res Function(SenderUiEvent_JoinRefused) _then;

/// Create a copy of SenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? code = null,}) {
  return _then(SenderUiEvent_JoinRefused(
code: null == code ? _self.code : code // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class SenderUiEvent_RemovedByHost extends SenderUiEvent {
  const SenderUiEvent_RemovedByHost(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is SenderUiEvent_RemovedByHost);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'SenderUiEvent.removedByHost()';
}


}




/// @nodoc


class SenderUiEvent_Joined extends SenderUiEvent {
  const SenderUiEvent_Joined(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is SenderUiEvent_Joined);
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
  const SenderUiEvent_ConnectionState({required this.state}): super._();
  

/// New state.
 final  UiConnectionState state;

/// Create a copy of SenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SenderUiEvent_ConnectionStateCopyWith<SenderUiEvent_ConnectionState> get copyWith => _$SenderUiEvent_ConnectionStateCopyWithImpl<SenderUiEvent_ConnectionState>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is SenderUiEvent_ConnectionState&&(identical(other.state, state) || other.state == state));
}


@override
int get hashCode {
    return Object.hash(runtimeType,state);
}

@override
String toString() {
    return 'SenderUiEvent.connectionState(state: $state)';
}


}

/// @nodoc
abstract mixin class $SenderUiEvent_ConnectionStateCopyWith<$Res> implements $SenderUiEventCopyWith<$Res> {
  factory $SenderUiEvent_ConnectionStateCopyWith(SenderUiEvent_ConnectionState value, $Res Function(SenderUiEvent_ConnectionState) _then) = _$SenderUiEvent_ConnectionStateCopyWithImpl;
@useResult
$Res call({
 UiConnectionState state
});




}
/// @nodoc
class _$SenderUiEvent_ConnectionStateCopyWithImpl<$Res>
    implements $SenderUiEvent_ConnectionStateCopyWith<$Res> {
  _$SenderUiEvent_ConnectionStateCopyWithImpl(this._self, this._then);

  final SenderUiEvent_ConnectionState _self;
  final $Res Function(SenderUiEvent_ConnectionState) _then;

/// Create a copy of SenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? state = null,}) {
  return _then(SenderUiEvent_ConnectionState(
state: null == state ? _self.state : state // ignore: cast_nullable_to_non_nullable
as UiConnectionState,
  ));
}


}

/// @nodoc


class SenderUiEvent_Streaming extends SenderUiEvent {
  const SenderUiEvent_Streaming({required this.framesSent}): super._();
  

/// Frames written to the track.
 final  BigInt framesSent;

/// Create a copy of SenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SenderUiEvent_StreamingCopyWith<SenderUiEvent_Streaming> get copyWith => _$SenderUiEvent_StreamingCopyWithImpl<SenderUiEvent_Streaming>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is SenderUiEvent_Streaming&&(identical(other.framesSent, framesSent) || other.framesSent == framesSent));
}


@override
int get hashCode {
    return Object.hash(runtimeType,framesSent);
}

@override
String toString() {
    return 'SenderUiEvent.streaming(framesSent: $framesSent)';
}


}

/// @nodoc
abstract mixin class $SenderUiEvent_StreamingCopyWith<$Res> implements $SenderUiEventCopyWith<$Res> {
  factory $SenderUiEvent_StreamingCopyWith(SenderUiEvent_Streaming value, $Res Function(SenderUiEvent_Streaming) _then) = _$SenderUiEvent_StreamingCopyWithImpl;
@useResult
$Res call({
 BigInt framesSent
});




}
/// @nodoc
class _$SenderUiEvent_StreamingCopyWithImpl<$Res>
    implements $SenderUiEvent_StreamingCopyWith<$Res> {
  _$SenderUiEvent_StreamingCopyWithImpl(this._self, this._then);

  final SenderUiEvent_Streaming _self;
  final $Res Function(SenderUiEvent_Streaming) _then;

/// Create a copy of SenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? framesSent = null,}) {
  return _then(SenderUiEvent_Streaming(
framesSent: null == framesSent ? _self.framesSent : framesSent // ignore: cast_nullable_to_non_nullable
as BigInt,
  ));
}


}

/// @nodoc


class SenderUiEvent_EncodedStats extends SenderUiEvent {
  const SenderUiEvent_EncodedStats({required this.pushed, required this.sent, required this.dropped, required this.keyframes, required this.keyframeRequests, required this.bytes}): super._();
  

/// Access units handed to [`push_encoded_frame`].
 final  BigInt pushed;
/// Units written to the transport.
 final  BigInt sent;
/// Units dropped (no peer yet, queue full, waiting for a key frame, no SPS/PPS,
/// replaced by a newer key frame, invalid).
 final  BigInt dropped;
/// Key frames written to the transport.
 final  BigInt keyframes;
/// Key frame requests (PLI, new peer, drop policy).
 final  BigInt keyframeRequests;
/// Bytes written to the transport.
 final  BigInt bytes;

/// Create a copy of SenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SenderUiEvent_EncodedStatsCopyWith<SenderUiEvent_EncodedStats> get copyWith => _$SenderUiEvent_EncodedStatsCopyWithImpl<SenderUiEvent_EncodedStats>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is SenderUiEvent_EncodedStats&&(identical(other.pushed, pushed) || other.pushed == pushed)&&(identical(other.sent, sent) || other.sent == sent)&&(identical(other.dropped, dropped) || other.dropped == dropped)&&(identical(other.keyframes, keyframes) || other.keyframes == keyframes)&&(identical(other.keyframeRequests, keyframeRequests) || other.keyframeRequests == keyframeRequests)&&(identical(other.bytes, bytes) || other.bytes == bytes));
}


@override
int get hashCode {
    return Object.hash(runtimeType,pushed,sent,dropped,keyframes,keyframeRequests,bytes);
}

@override
String toString() {
    return 'SenderUiEvent.encodedStats(pushed: $pushed, sent: $sent, dropped: $dropped, keyframes: $keyframes, keyframeRequests: $keyframeRequests, bytes: $bytes)';
}


}

/// @nodoc
abstract mixin class $SenderUiEvent_EncodedStatsCopyWith<$Res> implements $SenderUiEventCopyWith<$Res> {
  factory $SenderUiEvent_EncodedStatsCopyWith(SenderUiEvent_EncodedStats value, $Res Function(SenderUiEvent_EncodedStats) _then) = _$SenderUiEvent_EncodedStatsCopyWithImpl;
@useResult
$Res call({
 BigInt pushed, BigInt sent, BigInt dropped, BigInt keyframes, BigInt keyframeRequests, BigInt bytes
});




}
/// @nodoc
class _$SenderUiEvent_EncodedStatsCopyWithImpl<$Res>
    implements $SenderUiEvent_EncodedStatsCopyWith<$Res> {
  _$SenderUiEvent_EncodedStatsCopyWithImpl(this._self, this._then);

  final SenderUiEvent_EncodedStats _self;
  final $Res Function(SenderUiEvent_EncodedStats) _then;

/// Create a copy of SenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? pushed = null,Object? sent = null,Object? dropped = null,Object? keyframes = null,Object? keyframeRequests = null,Object? bytes = null,}) {
  return _then(SenderUiEvent_EncodedStats(
pushed: null == pushed ? _self.pushed : pushed // ignore: cast_nullable_to_non_nullable
as BigInt,sent: null == sent ? _self.sent : sent // ignore: cast_nullable_to_non_nullable
as BigInt,dropped: null == dropped ? _self.dropped : dropped // ignore: cast_nullable_to_non_nullable
as BigInt,keyframes: null == keyframes ? _self.keyframes : keyframes // ignore: cast_nullable_to_non_nullable
as BigInt,keyframeRequests: null == keyframeRequests ? _self.keyframeRequests : keyframeRequests // ignore: cast_nullable_to_non_nullable
as BigInt,bytes: null == bytes ? _self.bytes : bytes // ignore: cast_nullable_to_non_nullable
as BigInt,
  ));
}


}

/// @nodoc


class SenderUiEvent_ScreenStats extends SenderUiEvent {
  const SenderUiEvent_ScreenStats({required this.polls, required this.changed, required this.encoded, required this.keyframes, required this.encodedBytes}): super._();
  

/// Times the screen was looked at.
 final  BigInt polls;
/// Pictures that differed from the previous one.
 final  BigInt changed;
/// Frames produced by the encoder.
 final  BigInt encoded;
/// Of those, key frames.
 final  BigInt keyframes;
/// Encoded bytes so far.
 final  BigInt encodedBytes;

/// Create a copy of SenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SenderUiEvent_ScreenStatsCopyWith<SenderUiEvent_ScreenStats> get copyWith => _$SenderUiEvent_ScreenStatsCopyWithImpl<SenderUiEvent_ScreenStats>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is SenderUiEvent_ScreenStats&&(identical(other.polls, polls) || other.polls == polls)&&(identical(other.changed, changed) || other.changed == changed)&&(identical(other.encoded, encoded) || other.encoded == encoded)&&(identical(other.keyframes, keyframes) || other.keyframes == keyframes)&&(identical(other.encodedBytes, encodedBytes) || other.encodedBytes == encodedBytes));
}


@override
int get hashCode {
    return Object.hash(runtimeType,polls,changed,encoded,keyframes,encodedBytes);
}

@override
String toString() {
    return 'SenderUiEvent.screenStats(polls: $polls, changed: $changed, encoded: $encoded, keyframes: $keyframes, encodedBytes: $encodedBytes)';
}


}

/// @nodoc
abstract mixin class $SenderUiEvent_ScreenStatsCopyWith<$Res> implements $SenderUiEventCopyWith<$Res> {
  factory $SenderUiEvent_ScreenStatsCopyWith(SenderUiEvent_ScreenStats value, $Res Function(SenderUiEvent_ScreenStats) _then) = _$SenderUiEvent_ScreenStatsCopyWithImpl;
@useResult
$Res call({
 BigInt polls, BigInt changed, BigInt encoded, BigInt keyframes, BigInt encodedBytes
});




}
/// @nodoc
class _$SenderUiEvent_ScreenStatsCopyWithImpl<$Res>
    implements $SenderUiEvent_ScreenStatsCopyWith<$Res> {
  _$SenderUiEvent_ScreenStatsCopyWithImpl(this._self, this._then);

  final SenderUiEvent_ScreenStats _self;
  final $Res Function(SenderUiEvent_ScreenStats) _then;

/// Create a copy of SenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? polls = null,Object? changed = null,Object? encoded = null,Object? keyframes = null,Object? encodedBytes = null,}) {
  return _then(SenderUiEvent_ScreenStats(
polls: null == polls ? _self.polls : polls // ignore: cast_nullable_to_non_nullable
as BigInt,changed: null == changed ? _self.changed : changed // ignore: cast_nullable_to_non_nullable
as BigInt,encoded: null == encoded ? _self.encoded : encoded // ignore: cast_nullable_to_non_nullable
as BigInt,keyframes: null == keyframes ? _self.keyframes : keyframes // ignore: cast_nullable_to_non_nullable
as BigInt,encodedBytes: null == encodedBytes ? _self.encodedBytes : encodedBytes // ignore: cast_nullable_to_non_nullable
as BigInt,
  ));
}


}

/// @nodoc


class SenderUiEvent_Finished extends SenderUiEvent {
  const SenderUiEvent_Finished({required this.framesSent}): super._();
  

/// Frames written to the track.
 final  BigInt framesSent;

/// Create a copy of SenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SenderUiEvent_FinishedCopyWith<SenderUiEvent_Finished> get copyWith => _$SenderUiEvent_FinishedCopyWithImpl<SenderUiEvent_Finished>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is SenderUiEvent_Finished&&(identical(other.framesSent, framesSent) || other.framesSent == framesSent));
}


@override
int get hashCode {
    return Object.hash(runtimeType,framesSent);
}

@override
String toString() {
    return 'SenderUiEvent.finished(framesSent: $framesSent)';
}


}

/// @nodoc
abstract mixin class $SenderUiEvent_FinishedCopyWith<$Res> implements $SenderUiEventCopyWith<$Res> {
  factory $SenderUiEvent_FinishedCopyWith(SenderUiEvent_Finished value, $Res Function(SenderUiEvent_Finished) _then) = _$SenderUiEvent_FinishedCopyWithImpl;
@useResult
$Res call({
 BigInt framesSent
});




}
/// @nodoc
class _$SenderUiEvent_FinishedCopyWithImpl<$Res>
    implements $SenderUiEvent_FinishedCopyWith<$Res> {
  _$SenderUiEvent_FinishedCopyWithImpl(this._self, this._then);

  final SenderUiEvent_Finished _self;
  final $Res Function(SenderUiEvent_Finished) _then;

/// Create a copy of SenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? framesSent = null,}) {
  return _then(SenderUiEvent_Finished(
framesSent: null == framesSent ? _self.framesSent : framesSent // ignore: cast_nullable_to_non_nullable
as BigInt,
  ));
}


}

/// @nodoc


class SenderUiEvent_PeerLeft extends SenderUiEvent {
  const SenderUiEvent_PeerLeft(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is SenderUiEvent_PeerLeft);
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
  const SenderUiEvent_Error({required this.message}): super._();
  

/// Technical message in English.
 final  String message;

/// Create a copy of SenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SenderUiEvent_ErrorCopyWith<SenderUiEvent_Error> get copyWith => _$SenderUiEvent_ErrorCopyWithImpl<SenderUiEvent_Error>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is SenderUiEvent_Error&&(identical(other.message, message) || other.message == message));
}


@override
int get hashCode {
    return Object.hash(runtimeType,message);
}

@override
String toString() {
    return 'SenderUiEvent.error(message: $message)';
}


}

/// @nodoc
abstract mixin class $SenderUiEvent_ErrorCopyWith<$Res> implements $SenderUiEventCopyWith<$Res> {
  factory $SenderUiEvent_ErrorCopyWith(SenderUiEvent_Error value, $Res Function(SenderUiEvent_Error) _then) = _$SenderUiEvent_ErrorCopyWithImpl;
@useResult
$Res call({
 String message
});




}
/// @nodoc
class _$SenderUiEvent_ErrorCopyWithImpl<$Res>
    implements $SenderUiEvent_ErrorCopyWith<$Res> {
  _$SenderUiEvent_ErrorCopyWithImpl(this._self, this._then);

  final SenderUiEvent_Error _self;
  final $Res Function(SenderUiEvent_Error) _then;

/// Create a copy of SenderUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? message = null,}) {
  return _then(SenderUiEvent_Error(
message: null == message ? _self.message : message // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc
mixin _$TvUiEvent {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is TvUiEvent);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'TvUiEvent()';
}


}

/// @nodoc
class $TvUiEventCopyWith<$Res>  {
$TvUiEventCopyWith(TvUiEvent _, $Res Function(TvUiEvent) __);
}


/// Adds pattern-matching-related methods to [TvUiEvent].
extension TvUiEventPatterns on TvUiEvent {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( TvUiEvent_Seen value)?  seen,TResult Function( TvUiEvent_Gone value)?  gone,TResult Function( TvUiEvent_Unavailable value)?  unavailable,required TResult orElse(),}){
final _that = this;
switch (_that) {
case TvUiEvent_Seen() when seen != null:
return seen(_that);case TvUiEvent_Gone() when gone != null:
return gone(_that);case TvUiEvent_Unavailable() when unavailable != null:
return unavailable(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( TvUiEvent_Seen value)  seen,required TResult Function( TvUiEvent_Gone value)  gone,required TResult Function( TvUiEvent_Unavailable value)  unavailable,}){
final _that = this;
switch (_that) {
case TvUiEvent_Seen():
return seen(_that);case TvUiEvent_Gone():
return gone(_that);case TvUiEvent_Unavailable():
return unavailable(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( TvUiEvent_Seen value)?  seen,TResult? Function( TvUiEvent_Gone value)?  gone,TResult? Function( TvUiEvent_Unavailable value)?  unavailable,}){
final _that = this;
switch (_that) {
case TvUiEvent_Seen() when seen != null:
return seen(_that);case TvUiEvent_Gone() when gone != null:
return gone(_that);case TvUiEvent_Unavailable() when unavailable != null:
return unavailable(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String id,  String name,  TvKind kind,  List<String> addresses,  bool canScreen,  bool canPhoto,  bool canVideo)?  seen,TResult Function( String id)?  gone,TResult Function()?  unavailable,required TResult orElse(),}) {final _that = this;
switch (_that) {
case TvUiEvent_Seen() when seen != null:
return seen(_that.id,_that.name,_that.kind,_that.addresses,_that.canScreen,_that.canPhoto,_that.canVideo);case TvUiEvent_Gone() when gone != null:
return gone(_that.id);case TvUiEvent_Unavailable() when unavailable != null:
return unavailable();case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String id,  String name,  TvKind kind,  List<String> addresses,  bool canScreen,  bool canPhoto,  bool canVideo)  seen,required TResult Function( String id)  gone,required TResult Function()  unavailable,}) {final _that = this;
switch (_that) {
case TvUiEvent_Seen():
return seen(_that.id,_that.name,_that.kind,_that.addresses,_that.canScreen,_that.canPhoto,_that.canVideo);case TvUiEvent_Gone():
return gone(_that.id);case TvUiEvent_Unavailable():
return unavailable();}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String id,  String name,  TvKind kind,  List<String> addresses,  bool canScreen,  bool canPhoto,  bool canVideo)?  seen,TResult? Function( String id)?  gone,TResult? Function()?  unavailable,}) {final _that = this;
switch (_that) {
case TvUiEvent_Seen() when seen != null:
return seen(_that.id,_that.name,_that.kind,_that.addresses,_that.canScreen,_that.canPhoto,_that.canVideo);case TvUiEvent_Gone() when gone != null:
return gone(_that.id);case TvUiEvent_Unavailable() when unavailable != null:
return unavailable();case _:
  return null;

}
}

}

/// @nodoc


class TvUiEvent_Seen extends TvUiEvent {
  const TvUiEvent_Seen({required this.id, required this.name, required this.kind, required  List<String> addresses, required this.canScreen, required this.canPhoto, required this.canVideo}): _addresses = addresses,super._();
  

/// Stable id for [`start_tv_presenter`] (`cast:<id>` or `airplay:<id>`).
 final  String id;
/// The name the TV gives itself, cleaned.
 final  String name;
/// Cast or AirPlay.
 final  TvKind kind;
/// Its addresses as text. The app hides a TV whose address is also the address of a
/// Comet Cast screen in the nearby list (decision N84).
 final  List<String> _addresses;
/// Its addresses as text. The app hides a TV whose address is also the address of a
/// Comet Cast screen in the nearby list (decision N84).
 List<String> get addresses {
  if (_addresses is EqualUnmodifiableListView) return _addresses;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_addresses);
}

/// Can show the screen. For Cast false when the program has no trust anchors to check
/// the device ([`set_cast_trust_anchors`]) or the device has no screen.
 final  bool canScreen;
/// Can show a photo (AirPlay).
 final  bool canPhoto;
/// Can play a media link (AirPlay).
 final  bool canVideo;

/// Create a copy of TvUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$TvUiEvent_SeenCopyWith<TvUiEvent_Seen> get copyWith => _$TvUiEvent_SeenCopyWithImpl<TvUiEvent_Seen>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is TvUiEvent_Seen&&(identical(other.id, id) || other.id == id)&&(identical(other.name, name) || other.name == name)&&(identical(other.kind, kind) || other.kind == kind)&&const DeepCollectionEquality().equals(other.addresses, _addresses)&&(identical(other.canScreen, canScreen) || other.canScreen == canScreen)&&(identical(other.canPhoto, canPhoto) || other.canPhoto == canPhoto)&&(identical(other.canVideo, canVideo) || other.canVideo == canVideo));
}


@override
int get hashCode {
    return Object.hash(runtimeType,id,name,kind,const DeepCollectionEquality().hash(_addresses),canScreen,canPhoto,canVideo);
}

@override
String toString() {
    return 'TvUiEvent.seen(id: $id, name: $name, kind: $kind, addresses: $addresses, canScreen: $canScreen, canPhoto: $canPhoto, canVideo: $canVideo)';
}


}

/// @nodoc
abstract mixin class $TvUiEvent_SeenCopyWith<$Res> implements $TvUiEventCopyWith<$Res> {
  factory $TvUiEvent_SeenCopyWith(TvUiEvent_Seen value, $Res Function(TvUiEvent_Seen) _then) = _$TvUiEvent_SeenCopyWithImpl;
@useResult
$Res call({
 String id, String name, TvKind kind, List<String> addresses, bool canScreen, bool canPhoto, bool canVideo
});




}
/// @nodoc
class _$TvUiEvent_SeenCopyWithImpl<$Res>
    implements $TvUiEvent_SeenCopyWith<$Res> {
  _$TvUiEvent_SeenCopyWithImpl(this._self, this._then);

  final TvUiEvent_Seen _self;
  final $Res Function(TvUiEvent_Seen) _then;

/// Create a copy of TvUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? id = null,Object? name = null,Object? kind = null,Object? addresses = null,Object? canScreen = null,Object? canPhoto = null,Object? canVideo = null,}) {
  return _then(TvUiEvent_Seen(
id: null == id ? _self.id : id // ignore: cast_nullable_to_non_nullable
as String,name: null == name ? _self.name : name // ignore: cast_nullable_to_non_nullable
as String,kind: null == kind ? _self.kind : kind // ignore: cast_nullable_to_non_nullable
as TvKind,addresses: null == addresses ? _self._addresses : addresses // ignore: cast_nullable_to_non_nullable
as List<String>,canScreen: null == canScreen ? _self.canScreen : canScreen // ignore: cast_nullable_to_non_nullable
as bool,canPhoto: null == canPhoto ? _self.canPhoto : canPhoto // ignore: cast_nullable_to_non_nullable
as bool,canVideo: null == canVideo ? _self.canVideo : canVideo // ignore: cast_nullable_to_non_nullable
as bool,
  ));
}


}

/// @nodoc


class TvUiEvent_Gone extends TvUiEvent {
  const TvUiEvent_Gone({required this.id}): super._();
  

/// The id of its last `Seen`.
 final  String id;

/// Create a copy of TvUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$TvUiEvent_GoneCopyWith<TvUiEvent_Gone> get copyWith => _$TvUiEvent_GoneCopyWithImpl<TvUiEvent_Gone>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is TvUiEvent_Gone&&(identical(other.id, id) || other.id == id));
}


@override
int get hashCode {
    return Object.hash(runtimeType,id);
}

@override
String toString() {
    return 'TvUiEvent.gone(id: $id)';
}


}

/// @nodoc
abstract mixin class $TvUiEvent_GoneCopyWith<$Res> implements $TvUiEventCopyWith<$Res> {
  factory $TvUiEvent_GoneCopyWith(TvUiEvent_Gone value, $Res Function(TvUiEvent_Gone) _then) = _$TvUiEvent_GoneCopyWithImpl;
@useResult
$Res call({
 String id
});




}
/// @nodoc
class _$TvUiEvent_GoneCopyWithImpl<$Res>
    implements $TvUiEvent_GoneCopyWith<$Res> {
  _$TvUiEvent_GoneCopyWithImpl(this._self, this._then);

  final TvUiEvent_Gone _self;
  final $Res Function(TvUiEvent_Gone) _then;

/// Create a copy of TvUiEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? id = null,}) {
  return _then(TvUiEvent_Gone(
id: null == id ? _self.id : id // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class TvUiEvent_Unavailable extends TvUiEvent {
  const TvUiEvent_Unavailable(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is TvUiEvent_Unavailable);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'TvUiEvent.unavailable()';
}


}




/// @nodoc
mixin _$UiCodecState {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCodecState);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'UiCodecState()';
}


}

/// @nodoc
class $UiCodecStateCopyWith<$Res>  {
$UiCodecStateCopyWith(UiCodecState _, $Res Function(UiCodecState) __);
}


/// Adds pattern-matching-related methods to [UiCodecState].
extension UiCodecStatePatterns on UiCodecState {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( UiCodecState_Ready value)?  ready,TResult Function( UiCodecState_Missing value)?  missing,TResult Function( UiCodecState_Downloading value)?  downloading,TResult Function( UiCodecState_Failed value)?  failed,TResult Function( UiCodecState_Unsupported value)?  unsupported,required TResult orElse(),}){
final _that = this;
switch (_that) {
case UiCodecState_Ready() when ready != null:
return ready(_that);case UiCodecState_Missing() when missing != null:
return missing(_that);case UiCodecState_Downloading() when downloading != null:
return downloading(_that);case UiCodecState_Failed() when failed != null:
return failed(_that);case UiCodecState_Unsupported() when unsupported != null:
return unsupported(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( UiCodecState_Ready value)  ready,required TResult Function( UiCodecState_Missing value)  missing,required TResult Function( UiCodecState_Downloading value)  downloading,required TResult Function( UiCodecState_Failed value)  failed,required TResult Function( UiCodecState_Unsupported value)  unsupported,}){
final _that = this;
switch (_that) {
case UiCodecState_Ready():
return ready(_that);case UiCodecState_Missing():
return missing(_that);case UiCodecState_Downloading():
return downloading(_that);case UiCodecState_Failed():
return failed(_that);case UiCodecState_Unsupported():
return unsupported(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( UiCodecState_Ready value)?  ready,TResult? Function( UiCodecState_Missing value)?  missing,TResult? Function( UiCodecState_Downloading value)?  downloading,TResult? Function( UiCodecState_Failed value)?  failed,TResult? Function( UiCodecState_Unsupported value)?  unsupported,}){
final _that = this;
switch (_that) {
case UiCodecState_Ready() when ready != null:
return ready(_that);case UiCodecState_Missing() when missing != null:
return missing(_that);case UiCodecState_Downloading() when downloading != null:
return downloading(_that);case UiCodecState_Failed() when failed != null:
return failed(_that);case UiCodecState_Unsupported() when unsupported != null:
return unsupported(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  ready,TResult Function()?  missing,TResult Function( double progress)?  downloading,TResult Function( String reason)?  failed,TResult Function( String reason)?  unsupported,required TResult orElse(),}) {final _that = this;
switch (_that) {
case UiCodecState_Ready() when ready != null:
return ready();case UiCodecState_Missing() when missing != null:
return missing();case UiCodecState_Downloading() when downloading != null:
return downloading(_that.progress);case UiCodecState_Failed() when failed != null:
return failed(_that.reason);case UiCodecState_Unsupported() when unsupported != null:
return unsupported(_that.reason);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  ready,required TResult Function()  missing,required TResult Function( double progress)  downloading,required TResult Function( String reason)  failed,required TResult Function( String reason)  unsupported,}) {final _that = this;
switch (_that) {
case UiCodecState_Ready():
return ready();case UiCodecState_Missing():
return missing();case UiCodecState_Downloading():
return downloading(_that.progress);case UiCodecState_Failed():
return failed(_that.reason);case UiCodecState_Unsupported():
return unsupported(_that.reason);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  ready,TResult? Function()?  missing,TResult? Function( double progress)?  downloading,TResult? Function( String reason)?  failed,TResult? Function( String reason)?  unsupported,}) {final _that = this;
switch (_that) {
case UiCodecState_Ready() when ready != null:
return ready();case UiCodecState_Missing() when missing != null:
return missing();case UiCodecState_Downloading() when downloading != null:
return downloading(_that.progress);case UiCodecState_Failed() when failed != null:
return failed(_that.reason);case UiCodecState_Unsupported() when unsupported != null:
return unsupported(_that.reason);case _:
  return null;

}
}

}

/// @nodoc


class UiCodecState_Ready extends UiCodecState {
  const UiCodecState_Ready(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCodecState_Ready);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'UiCodecState.ready()';
}


}




/// @nodoc


class UiCodecState_Missing extends UiCodecState {
  const UiCodecState_Missing(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCodecState_Missing);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'UiCodecState.missing()';
}


}




/// @nodoc


class UiCodecState_Downloading extends UiCodecState {
  const UiCodecState_Downloading({required this.progress}): super._();
  

/// Fraction done.
 final  double progress;

/// Create a copy of UiCodecState
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCodecState_DownloadingCopyWith<UiCodecState_Downloading> get copyWith => _$UiCodecState_DownloadingCopyWithImpl<UiCodecState_Downloading>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCodecState_Downloading&&(identical(other.progress, progress) || other.progress == progress));
}


@override
int get hashCode {
    return Object.hash(runtimeType,progress);
}

@override
String toString() {
    return 'UiCodecState.downloading(progress: $progress)';
}


}

/// @nodoc
abstract mixin class $UiCodecState_DownloadingCopyWith<$Res> implements $UiCodecStateCopyWith<$Res> {
  factory $UiCodecState_DownloadingCopyWith(UiCodecState_Downloading value, $Res Function(UiCodecState_Downloading) _then) = _$UiCodecState_DownloadingCopyWithImpl;
@useResult
$Res call({
 double progress
});




}
/// @nodoc
class _$UiCodecState_DownloadingCopyWithImpl<$Res>
    implements $UiCodecState_DownloadingCopyWith<$Res> {
  _$UiCodecState_DownloadingCopyWithImpl(this._self, this._then);

  final UiCodecState_Downloading _self;
  final $Res Function(UiCodecState_Downloading) _then;

/// Create a copy of UiCodecState
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? progress = null,}) {
  return _then(UiCodecState_Downloading(
progress: null == progress ? _self.progress : progress // ignore: cast_nullable_to_non_nullable
as double,
  ));
}


}

/// @nodoc


class UiCodecState_Failed extends UiCodecState {
  const UiCodecState_Failed({required this.reason}): super._();
  

/// What went wrong.
 final  String reason;

/// Create a copy of UiCodecState
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCodecState_FailedCopyWith<UiCodecState_Failed> get copyWith => _$UiCodecState_FailedCopyWithImpl<UiCodecState_Failed>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCodecState_Failed&&(identical(other.reason, reason) || other.reason == reason));
}


@override
int get hashCode {
    return Object.hash(runtimeType,reason);
}

@override
String toString() {
    return 'UiCodecState.failed(reason: $reason)';
}


}

/// @nodoc
abstract mixin class $UiCodecState_FailedCopyWith<$Res> implements $UiCodecStateCopyWith<$Res> {
  factory $UiCodecState_FailedCopyWith(UiCodecState_Failed value, $Res Function(UiCodecState_Failed) _then) = _$UiCodecState_FailedCopyWithImpl;
@useResult
$Res call({
 String reason
});




}
/// @nodoc
class _$UiCodecState_FailedCopyWithImpl<$Res>
    implements $UiCodecState_FailedCopyWith<$Res> {
  _$UiCodecState_FailedCopyWithImpl(this._self, this._then);

  final UiCodecState_Failed _self;
  final $Res Function(UiCodecState_Failed) _then;

/// Create a copy of UiCodecState
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? reason = null,}) {
  return _then(UiCodecState_Failed(
reason: null == reason ? _self.reason : reason // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class UiCodecState_Unsupported extends UiCodecState {
  const UiCodecState_Unsupported({required this.reason}): super._();
  

/// Why.
 final  String reason;

/// Create a copy of UiCodecState
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiCodecState_UnsupportedCopyWith<UiCodecState_Unsupported> get copyWith => _$UiCodecState_UnsupportedCopyWithImpl<UiCodecState_Unsupported>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is UiCodecState_Unsupported&&(identical(other.reason, reason) || other.reason == reason));
}


@override
int get hashCode {
    return Object.hash(runtimeType,reason);
}

@override
String toString() {
    return 'UiCodecState.unsupported(reason: $reason)';
}


}

/// @nodoc
abstract mixin class $UiCodecState_UnsupportedCopyWith<$Res> implements $UiCodecStateCopyWith<$Res> {
  factory $UiCodecState_UnsupportedCopyWith(UiCodecState_Unsupported value, $Res Function(UiCodecState_Unsupported) _then) = _$UiCodecState_UnsupportedCopyWithImpl;
@useResult
$Res call({
 String reason
});




}
/// @nodoc
class _$UiCodecState_UnsupportedCopyWithImpl<$Res>
    implements $UiCodecState_UnsupportedCopyWith<$Res> {
  _$UiCodecState_UnsupportedCopyWithImpl(this._self, this._then);

  final UiCodecState_Unsupported _self;
  final $Res Function(UiCodecState_Unsupported) _then;

/// Create a copy of UiCodecState
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? reason = null,}) {
  return _then(UiCodecState_Unsupported(
reason: null == reason ? _self.reason : reason // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc
mixin _$UiContentCmd {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is UiContentCmd);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'UiContentCmd()';
}


}

/// @nodoc
class $UiContentCmdCopyWith<$Res>  {
$UiContentCmdCopyWith(UiContentCmd _, $Res Function(UiContentCmd) __);
}


/// Adds pattern-matching-related methods to [UiContentCmd].
extension UiContentCmdPatterns on UiContentCmd {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( UiContentCmd_NextPage value)?  nextPage,TResult Function( UiContentCmd_PrevPage value)?  prevPage,TResult Function( UiContentCmd_GoToPage value)?  goToPage,TResult Function( UiContentCmd_Stop value)?  stop,required TResult orElse(),}){
final _that = this;
switch (_that) {
case UiContentCmd_NextPage() when nextPage != null:
return nextPage(_that);case UiContentCmd_PrevPage() when prevPage != null:
return prevPage(_that);case UiContentCmd_GoToPage() when goToPage != null:
return goToPage(_that);case UiContentCmd_Stop() when stop != null:
return stop(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( UiContentCmd_NextPage value)  nextPage,required TResult Function( UiContentCmd_PrevPage value)  prevPage,required TResult Function( UiContentCmd_GoToPage value)  goToPage,required TResult Function( UiContentCmd_Stop value)  stop,}){
final _that = this;
switch (_that) {
case UiContentCmd_NextPage():
return nextPage(_that);case UiContentCmd_PrevPage():
return prevPage(_that);case UiContentCmd_GoToPage():
return goToPage(_that);case UiContentCmd_Stop():
return stop(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( UiContentCmd_NextPage value)?  nextPage,TResult? Function( UiContentCmd_PrevPage value)?  prevPage,TResult? Function( UiContentCmd_GoToPage value)?  goToPage,TResult? Function( UiContentCmd_Stop value)?  stop,}){
final _that = this;
switch (_that) {
case UiContentCmd_NextPage() when nextPage != null:
return nextPage(_that);case UiContentCmd_PrevPage() when prevPage != null:
return prevPage(_that);case UiContentCmd_GoToPage() when goToPage != null:
return goToPage(_that);case UiContentCmd_Stop() when stop != null:
return stop(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  nextPage,TResult Function()?  prevPage,TResult Function( int page)?  goToPage,TResult Function()?  stop,required TResult orElse(),}) {final _that = this;
switch (_that) {
case UiContentCmd_NextPage() when nextPage != null:
return nextPage();case UiContentCmd_PrevPage() when prevPage != null:
return prevPage();case UiContentCmd_GoToPage() when goToPage != null:
return goToPage(_that.page);case UiContentCmd_Stop() when stop != null:
return stop();case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  nextPage,required TResult Function()  prevPage,required TResult Function( int page)  goToPage,required TResult Function()  stop,}) {final _that = this;
switch (_that) {
case UiContentCmd_NextPage():
return nextPage();case UiContentCmd_PrevPage():
return prevPage();case UiContentCmd_GoToPage():
return goToPage(_that.page);case UiContentCmd_Stop():
return stop();}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  nextPage,TResult? Function()?  prevPage,TResult? Function( int page)?  goToPage,TResult? Function()?  stop,}) {final _that = this;
switch (_that) {
case UiContentCmd_NextPage() when nextPage != null:
return nextPage();case UiContentCmd_PrevPage() when prevPage != null:
return prevPage();case UiContentCmd_GoToPage() when goToPage != null:
return goToPage(_that.page);case UiContentCmd_Stop() when stop != null:
return stop();case _:
  return null;

}
}

}

/// @nodoc


class UiContentCmd_NextPage extends UiContentCmd {
  const UiContentCmd_NextPage(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is UiContentCmd_NextPage);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'UiContentCmd.nextPage()';
}


}




/// @nodoc


class UiContentCmd_PrevPage extends UiContentCmd {
  const UiContentCmd_PrevPage(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is UiContentCmd_PrevPage);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'UiContentCmd.prevPage()';
}


}




/// @nodoc


class UiContentCmd_GoToPage extends UiContentCmd {
  const UiContentCmd_GoToPage({required this.page}): super._();
  

/// Page number, 1-based.
 final  int page;

/// Create a copy of UiContentCmd
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$UiContentCmd_GoToPageCopyWith<UiContentCmd_GoToPage> get copyWith => _$UiContentCmd_GoToPageCopyWithImpl<UiContentCmd_GoToPage>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is UiContentCmd_GoToPage&&(identical(other.page, page) || other.page == page));
}


@override
int get hashCode {
    return Object.hash(runtimeType,page);
}

@override
String toString() {
    return 'UiContentCmd.goToPage(page: $page)';
}


}

/// @nodoc
abstract mixin class $UiContentCmd_GoToPageCopyWith<$Res> implements $UiContentCmdCopyWith<$Res> {
  factory $UiContentCmd_GoToPageCopyWith(UiContentCmd_GoToPage value, $Res Function(UiContentCmd_GoToPage) _then) = _$UiContentCmd_GoToPageCopyWithImpl;
@useResult
$Res call({
 int page
});




}
/// @nodoc
class _$UiContentCmd_GoToPageCopyWithImpl<$Res>
    implements $UiContentCmd_GoToPageCopyWith<$Res> {
  _$UiContentCmd_GoToPageCopyWithImpl(this._self, this._then);

  final UiContentCmd_GoToPage _self;
  final $Res Function(UiContentCmd_GoToPage) _then;

/// Create a copy of UiContentCmd
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? page = null,}) {
  return _then(UiContentCmd_GoToPage(
page: null == page ? _self.page : page // ignore: cast_nullable_to_non_nullable
as int,
  ));
}


}

/// @nodoc


class UiContentCmd_Stop extends UiContentCmd {
  const UiContentCmd_Stop(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is UiContentCmd_Stop);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'UiContentCmd.stop()';
}


}




// dart format on
