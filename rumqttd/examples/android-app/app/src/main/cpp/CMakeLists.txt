cmake_minimum_required(VERSION 3.22.1)
project("rumqttd_jni")

# 导入预编译的 rumqttd 静态库
add_library(rumqttd STATIC IMPORTED)
set_target_properties(rumqttd PROPERTIES
    IMPORTED_LOCATION ${CMAKE_SOURCE_DIR}/libs/${ANDROID_ABI}/librumqttd.a
)

# JNI 胶水层
add_library(rumqttd_jni SHARED
    RumqttdJNI.c
)

# 包含头文件目录（rumqttd.h 在同目录下）
target_include_directories(rumqttd_jni PRIVATE ${CMAKE_SOURCE_DIR})

# 链接
target_link_libraries(rumqttd_jni
    rumqttd
    log
)
