# Preserve exact JNI bridge signatures for libvexio_jni.so
-keep class com.vexio.core.NativeBridge {
    *;
}

-keepclasseswithmembernames,includedescriptorclasses class com.vexio.core.** {
    native <methods>;
}

# Preserve kotlinx.serialization descriptors for cross-window Drag & Drop payloads
-keepattributes *Annotation*, InnerClasses
-dontnote kotlinx.serialization.AnnotationsKt
-keepclassmembers class com.vexio.ui.workstation.** {
    *** Companion;
}
