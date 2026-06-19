cmake_minimum_required(VERSION 3.22.1)
project("rumqttc_jni")

add_library(rumqttc_static STATIC IMPORTED)
set_target_properties(rumqttc_static PROPERTIES
    IMPORTED_LOCATION ${CMAKE_SOURCE_DIR}/libs/${ANDROID_ABI}/librumqttc_android.a)

add_library(rumqttc_jni SHARED RumqttcJNI.c)
target_include_directories(rumqttc_jni PRIVATE ${CMAKE_SOURCE_DIR}/../../../src/android/include)
target_link_libraries(rumqttc_jni rumqttc_static log)
