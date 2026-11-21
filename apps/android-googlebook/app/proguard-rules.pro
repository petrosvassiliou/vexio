# Preserve exact JNI bridge signatures for libvexio_jni.so
-keep class com.vexio.core.NativeBridge {
    *;
}

-keepclasseswithmembernames,includedescriptorclasses class com.vexio.core.** {
    native <methods>;
}

# Preserve kotlinx.serialization descriptors for stencil DTOs and cross-window Drag & Drop payloads
-keepattributes *Annotation*, InnerClasses
-dontnote kotlinx.serialization.AnnotationsKt
-keepclassmembers class kotlinx.serialization.json.** {
    *** Companion;
}
-keepclasseswithmembers class kotlinx.serialization.json.** {
    kotlinx.serialization.KSerializer serializer(...);
}
-if @kotlinx.serialization.Serializable class **
-keepclassmembers class <1> {
    static <1>$Companion Companion;
}
-if @kotlinx.serialization.Serializable class ** {
    static **$* *;
}
-keepnames class <2>$$<3>
-if @kotlinx.serialization.Serializable class **
-keepclassmembers class <1>$Companion {
    kotlinx.serialization.KSerializer serializer(...);
}
-if @kotlinx.serialization.Serializable class **
-keepclasseswithmembers class <1>$$serializer {
    *;
}
-keep class com.vexio.core.NativeStencilMasterDto { *; }
-keep class com.vexio.core.StencilMasterItem { *; }
-keep class com.vexio.core.CrossWindowShapePayload { *; }

