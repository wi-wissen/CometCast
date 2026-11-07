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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( ContentSenderUiEvent_Prepared value)?  prepared,TResult Function( ContentSenderUiEvent_Converting value)?  converting,TResult Function( ContentSenderUiEvent_Joined value)?  joined,TResult Function( ContentSenderUiEvent_ConnectionState value)?  connectionState,TResult Function( ContentSenderUiEvent_Connected value)?  connected,TResult Function( ContentSenderUiEvent_Progress value)?  progress,TResult Function( ContentSenderUiEvent_Sent value)?  sent,TResult Function( ContentSenderUiEvent_Stopped value)?  stopped,TResult Function( ContentSenderUiEvent_PeerLeft value)?  peerLeft,TResult Function( ContentSenderUiEvent_Rejected value)?  rejected,TResult Function( ContentSenderUiEvent_Error value)?  error,required TResult orElse(),}){
final _that = this;
switch (_that) {
case ContentSenderUiEvent_Prepared() when prepared != null:
return prepared(_that);case ContentSenderUiEvent_Converting() when converting != null:
return converting(_that);case ContentSenderUiEvent_Joined() when joined != null:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( ContentSenderUiEvent_Prepared value)  prepared,required TResult Function( ContentSenderUiEvent_Converting value)  converting,required TResult Function( ContentSenderUiEvent_Joined value)  joined,required TResult Function( ContentSenderUiEvent_ConnectionState value)  connectionState,required TResult Function( ContentSenderUiEvent_Connected value)  connected,required TResult Function( ContentSenderUiEvent_Progress value)  progress,required TResult Function( ContentSenderUiEvent_Sent value)  sent,required TResult Function( ContentSenderUiEvent_Stopped value)  stopped,required TResult Function( ContentSenderUiEvent_PeerLeft value)  peerLeft,required TResult Function( ContentSenderUiEvent_Rejected value)  rejected,required TResult Function( ContentSenderUiEvent_Error value)  error,}){
final _that = this;
switch (_that) {
case ContentSenderUiEvent_Prepared():
return prepared(_that);case ContentSenderUiEvent_Converting():
return converting(_that);case ContentSenderUiEvent_Joined():
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( ContentSenderUiEvent_Prepared value)?  prepared,TResult? Function( ContentSenderUiEvent_Converting value)?  converting,TResult? Function( ContentSenderUiEvent_Joined value)?  joined,TResult? Function( ContentSenderUiEvent_ConnectionState value)?  connectionState,TResult? Function( ContentSenderUiEvent_Connected value)?  connected,TResult? Function( ContentSenderUiEvent_Progress value)?  progress,TResult? Function( ContentSenderUiEvent_Sent value)?  sent,TResult? Function( ContentSenderUiEvent_Stopped value)?  stopped,TResult? Function( ContentSenderUiEvent_PeerLeft value)?  peerLeft,TResult? Function( ContentSenderUiEvent_Rejected value)?  rejected,TResult? Function( ContentSenderUiEvent_Error value)?  error,}){
final _that = this;
switch (_that) {
case ContentSenderUiEvent_Prepared() when prepared != null:
return prepared(_that);case ContentSenderUiEvent_Converting() when converting != null:
return converting(_that);case ContentSenderUiEvent_Joined() when joined != null:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String name,  UiContentKind kind,  BigInt size,  int? pages,  String? host)?  prepared,TResult Function( String name)?  converting,TResult Function()?  joined,TResult Function( UiConnectionState state)?  connectionState,TResult Function()?  connected,TResult Function( BigInt sent,  BigInt total)?  progress,TResult Function()?  sent,TResult Function()?  stopped,TResult Function()?  peerLeft,TResult Function( UiSendRejection reason)?  rejected,TResult Function( String message)?  error,required TResult orElse(),}) {final _that = this;
switch (_that) {
case ContentSenderUiEvent_Prepared() when prepared != null:
return prepared(_that.name,_that.kind,_that.size,_that.pages,_that.host);case ContentSenderUiEvent_Converting() when converting != null:
return converting(_that.name);case ContentSenderUiEvent_Joined() when joined != null:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String name,  UiContentKind kind,  BigInt size,  int? pages,  String? host)  prepared,required TResult Function( String name)  converting,required TResult Function()  joined,required TResult Function( UiConnectionState state)  connectionState,required TResult Function()  connected,required TResult Function( BigInt sent,  BigInt total)  progress,required TResult Function()  sent,required TResult Function()  stopped,required TResult Function()  peerLeft,required TResult Function( UiSendRejection reason)  rejected,required TResult Function( String message)  error,}) {final _that = this;
switch (_that) {
case ContentSenderUiEvent_Prepared():
return prepared(_that.name,_that.kind,_that.size,_that.pages,_that.host);case ContentSenderUiEvent_Converting():
return converting(_that.name);case ContentSenderUiEvent_Joined():
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String name,  UiContentKind kind,  BigInt size,  int? pages,  String? host)?  prepared,TResult? Function( String name)?  converting,TResult? Function()?  joined,TResult? Function( UiConnectionState state)?  connectionState,TResult? Function()?  connected,TResult? Function( BigInt sent,  BigInt total)?  progress,TResult? Function()?  sent,TResult? Function()?  stopped,TResult? Function()?  peerLeft,TResult? Function( UiSendRejection reason)?  rejected,TResult? Function( String message)?  error,}) {final _that = this;
switch (_that) {
case ContentSenderUiEvent_Prepared() when prepared != null:
return prepared(_that.name,_that.kind,_that.size,_that.pages,_that.host);case ContentSenderUiEvent_Converting() when converting != null:
return converting(_that.name);case ContentSenderUiEvent_Joined() when joined != null:
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( ReceiverUiEvent_RoomCode value)?  roomCode,TResult Function( ReceiverUiEvent_PeerJoined value)?  peerJoined,TResult Function( ReceiverUiEvent_ConnectionState value)?  connectionState,TResult Function( ReceiverUiEvent_VideoTrack value)?  videoTrack,TResult Function( ReceiverUiEvent_Stats value)?  stats,TResult Function( ReceiverUiEvent_FrameReady value)?  frameReady,TResult Function( ReceiverUiEvent_EncodedReady value)?  encodedReady,TResult Function( ReceiverUiEvent_VideoStats value)?  videoStats,TResult Function( ReceiverUiEvent_ContentTransferStarted value)?  contentTransferStarted,TResult Function( ReceiverUiEvent_ContentFrameReady value)?  contentFrameReady,TResult Function( ReceiverUiEvent_ContentDocumentOpened value)?  contentDocumentOpened,TResult Function( ReceiverUiEvent_ContentLink value)?  contentLink,TResult Function( ReceiverUiEvent_ContentStopped value)?  contentStopped,TResult Function( ReceiverUiEvent_ContentError value)?  contentError,TResult Function( ReceiverUiEvent_PeerLeft value)?  peerLeft,TResult Function( ReceiverUiEvent_Error value)?  error,required TResult orElse(),}){
final _that = this;
switch (_that) {
case ReceiverUiEvent_RoomCode() when roomCode != null:
return roomCode(_that);case ReceiverUiEvent_PeerJoined() when peerJoined != null:
return peerJoined(_that);case ReceiverUiEvent_ConnectionState() when connectionState != null:
return connectionState(_that);case ReceiverUiEvent_VideoTrack() when videoTrack != null:
return videoTrack(_that);case ReceiverUiEvent_Stats() when stats != null:
return stats(_that);case ReceiverUiEvent_FrameReady() when frameReady != null:
return frameReady(_that);case ReceiverUiEvent_EncodedReady() when encodedReady != null:
return encodedReady(_that);case ReceiverUiEvent_VideoStats() when videoStats != null:
return videoStats(_that);case ReceiverUiEvent_ContentTransferStarted() when contentTransferStarted != null:
return contentTransferStarted(_that);case ReceiverUiEvent_ContentFrameReady() when contentFrameReady != null:
return contentFrameReady(_that);case ReceiverUiEvent_ContentDocumentOpened() when contentDocumentOpened != null:
return contentDocumentOpened(_that);case ReceiverUiEvent_ContentLink() when contentLink != null:
return contentLink(_that);case ReceiverUiEvent_ContentStopped() when contentStopped != null:
return contentStopped(_that);case ReceiverUiEvent_ContentError() when contentError != null:
return contentError(_that);case ReceiverUiEvent_PeerLeft() when peerLeft != null:
return peerLeft(_that);case ReceiverUiEvent_Error() when error != null:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( ReceiverUiEvent_RoomCode value)  roomCode,required TResult Function( ReceiverUiEvent_PeerJoined value)  peerJoined,required TResult Function( ReceiverUiEvent_ConnectionState value)  connectionState,required TResult Function( ReceiverUiEvent_VideoTrack value)  videoTrack,required TResult Function( ReceiverUiEvent_Stats value)  stats,required TResult Function( ReceiverUiEvent_FrameReady value)  frameReady,required TResult Function( ReceiverUiEvent_EncodedReady value)  encodedReady,required TResult Function( ReceiverUiEvent_VideoStats value)  videoStats,required TResult Function( ReceiverUiEvent_ContentTransferStarted value)  contentTransferStarted,required TResult Function( ReceiverUiEvent_ContentFrameReady value)  contentFrameReady,required TResult Function( ReceiverUiEvent_ContentDocumentOpened value)  contentDocumentOpened,required TResult Function( ReceiverUiEvent_ContentLink value)  contentLink,required TResult Function( ReceiverUiEvent_ContentStopped value)  contentStopped,required TResult Function( ReceiverUiEvent_ContentError value)  contentError,required TResult Function( ReceiverUiEvent_PeerLeft value)  peerLeft,required TResult Function( ReceiverUiEvent_Error value)  error,}){
final _that = this;
switch (_that) {
case ReceiverUiEvent_RoomCode():
return roomCode(_that);case ReceiverUiEvent_PeerJoined():
return peerJoined(_that);case ReceiverUiEvent_ConnectionState():
return connectionState(_that);case ReceiverUiEvent_VideoTrack():
return videoTrack(_that);case ReceiverUiEvent_Stats():
return stats(_that);case ReceiverUiEvent_FrameReady():
return frameReady(_that);case ReceiverUiEvent_EncodedReady():
return encodedReady(_that);case ReceiverUiEvent_VideoStats():
return videoStats(_that);case ReceiverUiEvent_ContentTransferStarted():
return contentTransferStarted(_that);case ReceiverUiEvent_ContentFrameReady():
return contentFrameReady(_that);case ReceiverUiEvent_ContentDocumentOpened():
return contentDocumentOpened(_that);case ReceiverUiEvent_ContentLink():
return contentLink(_that);case ReceiverUiEvent_ContentStopped():
return contentStopped(_that);case ReceiverUiEvent_ContentError():
return contentError(_that);case ReceiverUiEvent_PeerLeft():
return peerLeft(_that);case ReceiverUiEvent_Error():
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( ReceiverUiEvent_RoomCode value)?  roomCode,TResult? Function( ReceiverUiEvent_PeerJoined value)?  peerJoined,TResult? Function( ReceiverUiEvent_ConnectionState value)?  connectionState,TResult? Function( ReceiverUiEvent_VideoTrack value)?  videoTrack,TResult? Function( ReceiverUiEvent_Stats value)?  stats,TResult? Function( ReceiverUiEvent_FrameReady value)?  frameReady,TResult? Function( ReceiverUiEvent_EncodedReady value)?  encodedReady,TResult? Function( ReceiverUiEvent_VideoStats value)?  videoStats,TResult? Function( ReceiverUiEvent_ContentTransferStarted value)?  contentTransferStarted,TResult? Function( ReceiverUiEvent_ContentFrameReady value)?  contentFrameReady,TResult? Function( ReceiverUiEvent_ContentDocumentOpened value)?  contentDocumentOpened,TResult? Function( ReceiverUiEvent_ContentLink value)?  contentLink,TResult? Function( ReceiverUiEvent_ContentStopped value)?  contentStopped,TResult? Function( ReceiverUiEvent_ContentError value)?  contentError,TResult? Function( ReceiverUiEvent_PeerLeft value)?  peerLeft,TResult? Function( ReceiverUiEvent_Error value)?  error,}){
final _that = this;
switch (_that) {
case ReceiverUiEvent_RoomCode() when roomCode != null:
return roomCode(_that);case ReceiverUiEvent_PeerJoined() when peerJoined != null:
return peerJoined(_that);case ReceiverUiEvent_ConnectionState() when connectionState != null:
return connectionState(_that);case ReceiverUiEvent_VideoTrack() when videoTrack != null:
return videoTrack(_that);case ReceiverUiEvent_Stats() when stats != null:
return stats(_that);case ReceiverUiEvent_FrameReady() when frameReady != null:
return frameReady(_that);case ReceiverUiEvent_EncodedReady() when encodedReady != null:
return encodedReady(_that);case ReceiverUiEvent_VideoStats() when videoStats != null:
return videoStats(_that);case ReceiverUiEvent_ContentTransferStarted() when contentTransferStarted != null:
return contentTransferStarted(_that);case ReceiverUiEvent_ContentFrameReady() when contentFrameReady != null:
return contentFrameReady(_that);case ReceiverUiEvent_ContentDocumentOpened() when contentDocumentOpened != null:
return contentDocumentOpened(_that);case ReceiverUiEvent_ContentLink() when contentLink != null:
return contentLink(_that);case ReceiverUiEvent_ContentStopped() when contentStopped != null:
return contentStopped(_that);case ReceiverUiEvent_ContentError() when contentError != null:
return contentError(_that);case ReceiverUiEvent_PeerLeft() when peerLeft != null:
return peerLeft(_that);case ReceiverUiEvent_Error() when error != null:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String code)?  roomCode,TResult Function()?  peerJoined,TResult Function( UiConnectionState state)?  connectionState,TResult Function( String mime,  String? codec)?  videoTrack,TResult Function( BigInt packets,  BigInt bytes,  BigInt frames,  BigInt keyframes)?  stats,TResult Function()?  frameReady,TResult Function()?  encodedReady,TResult Function( BigInt decoded,  BigInt delivered,  BigInt dropped,  BigInt droppedPackets,  BigInt decodeErrors,  BigInt keyframeRequests,  int width,  int height,  BigInt avgDecodeUs,  String? error)?  videoStats,TResult Function( bool document,  String name,  BigInt size)?  contentTransferStarted,TResult Function()?  contentFrameReady,TResult Function( String name,  int pages)?  contentDocumentOpened,TResult Function( String url,  String host)?  contentLink,TResult Function()?  contentStopped,TResult Function( UiContentError error)?  contentError,TResult Function()?  peerLeft,TResult Function( String message)?  error,required TResult orElse(),}) {final _that = this;
switch (_that) {
case ReceiverUiEvent_RoomCode() when roomCode != null:
return roomCode(_that.code);case ReceiverUiEvent_PeerJoined() when peerJoined != null:
return peerJoined();case ReceiverUiEvent_ConnectionState() when connectionState != null:
return connectionState(_that.state);case ReceiverUiEvent_VideoTrack() when videoTrack != null:
return videoTrack(_that.mime,_that.codec);case ReceiverUiEvent_Stats() when stats != null:
return stats(_that.packets,_that.bytes,_that.frames,_that.keyframes);case ReceiverUiEvent_FrameReady() when frameReady != null:
return frameReady();case ReceiverUiEvent_EncodedReady() when encodedReady != null:
return encodedReady();case ReceiverUiEvent_VideoStats() when videoStats != null:
return videoStats(_that.decoded,_that.delivered,_that.dropped,_that.droppedPackets,_that.decodeErrors,_that.keyframeRequests,_that.width,_that.height,_that.avgDecodeUs,_that.error);case ReceiverUiEvent_ContentTransferStarted() when contentTransferStarted != null:
return contentTransferStarted(_that.document,_that.name,_that.size);case ReceiverUiEvent_ContentFrameReady() when contentFrameReady != null:
return contentFrameReady();case ReceiverUiEvent_ContentDocumentOpened() when contentDocumentOpened != null:
return contentDocumentOpened(_that.name,_that.pages);case ReceiverUiEvent_ContentLink() when contentLink != null:
return contentLink(_that.url,_that.host);case ReceiverUiEvent_ContentStopped() when contentStopped != null:
return contentStopped();case ReceiverUiEvent_ContentError() when contentError != null:
return contentError(_that.error);case ReceiverUiEvent_PeerLeft() when peerLeft != null:
return peerLeft();case ReceiverUiEvent_Error() when error != null:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String code)  roomCode,required TResult Function()  peerJoined,required TResult Function( UiConnectionState state)  connectionState,required TResult Function( String mime,  String? codec)  videoTrack,required TResult Function( BigInt packets,  BigInt bytes,  BigInt frames,  BigInt keyframes)  stats,required TResult Function()  frameReady,required TResult Function()  encodedReady,required TResult Function( BigInt decoded,  BigInt delivered,  BigInt dropped,  BigInt droppedPackets,  BigInt decodeErrors,  BigInt keyframeRequests,  int width,  int height,  BigInt avgDecodeUs,  String? error)  videoStats,required TResult Function( bool document,  String name,  BigInt size)  contentTransferStarted,required TResult Function()  contentFrameReady,required TResult Function( String name,  int pages)  contentDocumentOpened,required TResult Function( String url,  String host)  contentLink,required TResult Function()  contentStopped,required TResult Function( UiContentError error)  contentError,required TResult Function()  peerLeft,required TResult Function( String message)  error,}) {final _that = this;
switch (_that) {
case ReceiverUiEvent_RoomCode():
return roomCode(_that.code);case ReceiverUiEvent_PeerJoined():
return peerJoined();case ReceiverUiEvent_ConnectionState():
return connectionState(_that.state);case ReceiverUiEvent_VideoTrack():
return videoTrack(_that.mime,_that.codec);case ReceiverUiEvent_Stats():
return stats(_that.packets,_that.bytes,_that.frames,_that.keyframes);case ReceiverUiEvent_FrameReady():
return frameReady();case ReceiverUiEvent_EncodedReady():
return encodedReady();case ReceiverUiEvent_VideoStats():
return videoStats(_that.decoded,_that.delivered,_that.dropped,_that.droppedPackets,_that.decodeErrors,_that.keyframeRequests,_that.width,_that.height,_that.avgDecodeUs,_that.error);case ReceiverUiEvent_ContentTransferStarted():
return contentTransferStarted(_that.document,_that.name,_that.size);case ReceiverUiEvent_ContentFrameReady():
return contentFrameReady();case ReceiverUiEvent_ContentDocumentOpened():
return contentDocumentOpened(_that.name,_that.pages);case ReceiverUiEvent_ContentLink():
return contentLink(_that.url,_that.host);case ReceiverUiEvent_ContentStopped():
return contentStopped();case ReceiverUiEvent_ContentError():
return contentError(_that.error);case ReceiverUiEvent_PeerLeft():
return peerLeft();case ReceiverUiEvent_Error():
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String code)?  roomCode,TResult? Function()?  peerJoined,TResult? Function( UiConnectionState state)?  connectionState,TResult? Function( String mime,  String? codec)?  videoTrack,TResult? Function( BigInt packets,  BigInt bytes,  BigInt frames,  BigInt keyframes)?  stats,TResult? Function()?  frameReady,TResult? Function()?  encodedReady,TResult? Function( BigInt decoded,  BigInt delivered,  BigInt dropped,  BigInt droppedPackets,  BigInt decodeErrors,  BigInt keyframeRequests,  int width,  int height,  BigInt avgDecodeUs,  String? error)?  videoStats,TResult? Function( bool document,  String name,  BigInt size)?  contentTransferStarted,TResult? Function()?  contentFrameReady,TResult? Function( String name,  int pages)?  contentDocumentOpened,TResult? Function( String url,  String host)?  contentLink,TResult? Function()?  contentStopped,TResult? Function( UiContentError error)?  contentError,TResult? Function()?  peerLeft,TResult? Function( String message)?  error,}) {final _that = this;
switch (_that) {
case ReceiverUiEvent_RoomCode() when roomCode != null:
return roomCode(_that.code);case ReceiverUiEvent_PeerJoined() when peerJoined != null:
return peerJoined();case ReceiverUiEvent_ConnectionState() when connectionState != null:
return connectionState(_that.state);case ReceiverUiEvent_VideoTrack() when videoTrack != null:
return videoTrack(_that.mime,_that.codec);case ReceiverUiEvent_Stats() when stats != null:
return stats(_that.packets,_that.bytes,_that.frames,_that.keyframes);case ReceiverUiEvent_FrameReady() when frameReady != null:
return frameReady();case ReceiverUiEvent_EncodedReady() when encodedReady != null:
return encodedReady();case ReceiverUiEvent_VideoStats() when videoStats != null:
return videoStats(_that.decoded,_that.delivered,_that.dropped,_that.droppedPackets,_that.decodeErrors,_that.keyframeRequests,_that.width,_that.height,_that.avgDecodeUs,_that.error);case ReceiverUiEvent_ContentTransferStarted() when contentTransferStarted != null:
return contentTransferStarted(_that.document,_that.name,_that.size);case ReceiverUiEvent_ContentFrameReady() when contentFrameReady != null:
return contentFrameReady();case ReceiverUiEvent_ContentDocumentOpened() when contentDocumentOpened != null:
return contentDocumentOpened(_that.name,_that.pages);case ReceiverUiEvent_ContentLink() when contentLink != null:
return contentLink(_that.url,_that.host);case ReceiverUiEvent_ContentStopped() when contentStopped != null:
return contentStopped();case ReceiverUiEvent_ContentError() when contentError != null:
return contentError(_that.error);case ReceiverUiEvent_PeerLeft() when peerLeft != null:
return peerLeft();case ReceiverUiEvent_Error() when error != null:
return error(_that.message);case _:
  return null;

}
}

}

/// @nodoc


class ReceiverUiEvent_RoomCode extends ReceiverUiEvent {
  const ReceiverUiEvent_RoomCode({required this.code}): super._();
  

/// The room code, uppercase.
 final  String code;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiverUiEvent_RoomCodeCopyWith<ReceiverUiEvent_RoomCode> get copyWith => _$ReceiverUiEvent_RoomCodeCopyWithImpl<ReceiverUiEvent_RoomCode>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_RoomCode&&(identical(other.code, code) || other.code == code));
}


@override
int get hashCode {
    return Object.hash(runtimeType,code);
}

@override
String toString() {
    return 'ReceiverUiEvent.roomCode(code: $code)';
}


}

/// @nodoc
abstract mixin class $ReceiverUiEvent_RoomCodeCopyWith<$Res> implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_RoomCodeCopyWith(ReceiverUiEvent_RoomCode value, $Res Function(ReceiverUiEvent_RoomCode) _then) = _$ReceiverUiEvent_RoomCodeCopyWithImpl;
@useResult
$Res call({
 String code
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
@pragma('vm:prefer-inline') $Res call({Object? code = null,}) {
  return _then(ReceiverUiEvent_RoomCode(
code: null == code ? _self.code : code // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ReceiverUiEvent_PeerJoined extends ReceiverUiEvent {
  const ReceiverUiEvent_PeerJoined(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_PeerJoined);
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


class ReceiverUiEvent_Error extends ReceiverUiEvent {
  const ReceiverUiEvent_Error({required this.message}): super._();
  

/// Technical message in English (for logs and for showing next to a translated title).
 final  String message;

/// Create a copy of ReceiverUiEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiverUiEvent_ErrorCopyWith<ReceiverUiEvent_Error> get copyWith => _$ReceiverUiEvent_ErrorCopyWithImpl<ReceiverUiEvent_Error>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiverUiEvent_Error&&(identical(other.message, message) || other.message == message));
}


@override
int get hashCode {
    return Object.hash(runtimeType,message);
}

@override
String toString() {
    return 'ReceiverUiEvent.error(message: $message)';
}


}

/// @nodoc
abstract mixin class $ReceiverUiEvent_ErrorCopyWith<$Res> implements $ReceiverUiEventCopyWith<$Res> {
  factory $ReceiverUiEvent_ErrorCopyWith(ReceiverUiEvent_Error value, $Res Function(ReceiverUiEvent_Error) _then) = _$ReceiverUiEvent_ErrorCopyWithImpl;
@useResult
$Res call({
 String message
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
@pragma('vm:prefer-inline') $Res call({Object? message = null,}) {
  return _then(ReceiverUiEvent_Error(
message: null == message ? _self.message : message // ignore: cast_nullable_to_non_nullable
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( SenderUiEvent_Joined value)?  joined,TResult Function( SenderUiEvent_ConnectionState value)?  connectionState,TResult Function( SenderUiEvent_Streaming value)?  streaming,TResult Function( SenderUiEvent_EncodedStats value)?  encodedStats,TResult Function( SenderUiEvent_ScreenStats value)?  screenStats,TResult Function( SenderUiEvent_Finished value)?  finished,TResult Function( SenderUiEvent_PeerLeft value)?  peerLeft,TResult Function( SenderUiEvent_Error value)?  error,required TResult orElse(),}){
final _that = this;
switch (_that) {
case SenderUiEvent_Joined() when joined != null:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( SenderUiEvent_Joined value)  joined,required TResult Function( SenderUiEvent_ConnectionState value)  connectionState,required TResult Function( SenderUiEvent_Streaming value)  streaming,required TResult Function( SenderUiEvent_EncodedStats value)  encodedStats,required TResult Function( SenderUiEvent_ScreenStats value)  screenStats,required TResult Function( SenderUiEvent_Finished value)  finished,required TResult Function( SenderUiEvent_PeerLeft value)  peerLeft,required TResult Function( SenderUiEvent_Error value)  error,}){
final _that = this;
switch (_that) {
case SenderUiEvent_Joined():
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( SenderUiEvent_Joined value)?  joined,TResult? Function( SenderUiEvent_ConnectionState value)?  connectionState,TResult? Function( SenderUiEvent_Streaming value)?  streaming,TResult? Function( SenderUiEvent_EncodedStats value)?  encodedStats,TResult? Function( SenderUiEvent_ScreenStats value)?  screenStats,TResult? Function( SenderUiEvent_Finished value)?  finished,TResult? Function( SenderUiEvent_PeerLeft value)?  peerLeft,TResult? Function( SenderUiEvent_Error value)?  error,}){
final _that = this;
switch (_that) {
case SenderUiEvent_Joined() when joined != null:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  joined,TResult Function( UiConnectionState state)?  connectionState,TResult Function( BigInt framesSent)?  streaming,TResult Function( BigInt pushed,  BigInt sent,  BigInt dropped,  BigInt keyframes,  BigInt keyframeRequests,  BigInt bytes)?  encodedStats,TResult Function( BigInt polls,  BigInt changed,  BigInt encoded,  BigInt keyframes,  BigInt encodedBytes)?  screenStats,TResult Function( BigInt framesSent)?  finished,TResult Function()?  peerLeft,TResult Function( String message)?  error,required TResult orElse(),}) {final _that = this;
switch (_that) {
case SenderUiEvent_Joined() when joined != null:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  joined,required TResult Function( UiConnectionState state)  connectionState,required TResult Function( BigInt framesSent)  streaming,required TResult Function( BigInt pushed,  BigInt sent,  BigInt dropped,  BigInt keyframes,  BigInt keyframeRequests,  BigInt bytes)  encodedStats,required TResult Function( BigInt polls,  BigInt changed,  BigInt encoded,  BigInt keyframes,  BigInt encodedBytes)  screenStats,required TResult Function( BigInt framesSent)  finished,required TResult Function()  peerLeft,required TResult Function( String message)  error,}) {final _that = this;
switch (_that) {
case SenderUiEvent_Joined():
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  joined,TResult? Function( UiConnectionState state)?  connectionState,TResult? Function( BigInt framesSent)?  streaming,TResult? Function( BigInt pushed,  BigInt sent,  BigInt dropped,  BigInt keyframes,  BigInt keyframeRequests,  BigInt bytes)?  encodedStats,TResult? Function( BigInt polls,  BigInt changed,  BigInt encoded,  BigInt keyframes,  BigInt encodedBytes)?  screenStats,TResult? Function( BigInt framesSent)?  finished,TResult? Function()?  peerLeft,TResult? Function( String message)?  error,}) {final _that = this;
switch (_that) {
case SenderUiEvent_Joined() when joined != null:
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
