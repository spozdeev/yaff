set(YAFF_INTEROP_GENERATED_DIR ${CMAKE_CURRENT_BINARY_DIR}/generated)
file(MAKE_DIRECTORY ${YAFF_INTEROP_GENERATED_DIR})
file(GENERATE OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/protoc-path.txt CONTENT "$<TARGET_FILE:protobuf::protoc>")

add_library(yaff_interop_schemas STATIC schema.proto defaults.proto)
protobuf_generate(
    TARGET yaff_interop_schemas LANGUAGE cpp
    IMPORT_DIRS ${CMAKE_CURRENT_SOURCE_DIR} ${YAFF_PROTO_IMPORT_DIR}
    PROTOC_OUT_DIR ${YAFF_INTEROP_GENERATED_DIR}
)
yaff_generate(
    TARGET yaff_interop_schemas
    IMPORT_DIRS ${CMAKE_CURRENT_SOURCE_DIR}
    OUT_DIR ${YAFF_INTEROP_GENERATED_DIR}
)
yaff_generate(
    TARGET yaff_interop_schemas TAG light NAMESPACE light
    IMPORT_DIRS ${CMAKE_CURRENT_SOURCE_DIR}
    OUT_DIR ${YAFF_INTEROP_GENERATED_DIR}
)
# Imported WKT code is supplied by libprotobuf; only its YaFF view is generated.
get_target_property(_yaff_protobuf_includes protobuf::libprotobuf INTERFACE_INCLUDE_DIRECTORIES)
find_path(YAFF_WKT_IMPORT_DIR google/protobuf/timestamp.proto HINTS ${_yaff_protobuf_includes} REQUIRED)
add_custom_command(
    OUTPUT ${YAFF_INTEROP_GENERATED_DIR}/google/protobuf/timestamp.yaff.h
    COMMAND protobuf::protoc
        --plugin=protoc-gen-yaff=$<TARGET_FILE:yaff_protoc_plugin>
        --yaff_out=${YAFF_INTEROP_GENERATED_DIR}
        -I ${YAFF_WKT_IMPORT_DIR}
        google/protobuf/timestamp.proto
    DEPENDS yaff_protoc_plugin
)
add_custom_target(yaff_interop_wkt DEPENDS ${YAFF_INTEROP_GENERATED_DIR}/google/protobuf/timestamp.yaff.h)
add_dependencies(yaff_interop_schemas yaff_interop_wkt)
target_include_directories(yaff_interop_schemas PUBLIC ${YAFF_INTEROP_GENERATED_DIR})
target_link_libraries(yaff_interop_schemas PUBLIC yaff::core yaff::proto)

add_executable(yaff_generated_interop generated.cpp)
target_link_libraries(yaff_generated_interop PRIVATE yaff_interop_schemas)
