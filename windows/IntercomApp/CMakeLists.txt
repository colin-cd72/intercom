cmake_minimum_required(VERSION 3.16)
project(IntercomApp VERSION 1.0.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

# Find Qt6
find_package(Qt6 REQUIRED COMPONENTS Core Gui Widgets Multimedia)

# Include the Rust FFI library
set(INTERCOM_LIB_DIR "${CMAKE_SOURCE_DIR}/../IntercomCore")
set(INTERCOM_INCLUDE_DIR "${INTERCOM_LIB_DIR}/include")
set(INTERCOM_LIBRARY "${INTERCOM_LIB_DIR}/lib/intercom_ffi.lib")

# Source files
set(SOURCES
    src/main.cpp
    src/MainWindow.cpp
    src/IntercomClient.cpp
    src/ChannelListWidget.cpp
    src/UserListWidget.cpp
    src/TalkButtonWidget.cpp
    src/DeviceSettingsDialog.cpp
    src/ConnectDialog.cpp
)

set(HEADERS
    src/MainWindow.h
    src/IntercomClient.h
    src/ChannelListWidget.h
    src/UserListWidget.h
    src/TalkButtonWidget.h
    src/DeviceSettingsDialog.h
    src/ConnectDialog.h
)

# Create executable
add_executable(${PROJECT_NAME} ${SOURCES} ${HEADERS})

# Include directories
target_include_directories(${PROJECT_NAME} PRIVATE
    ${CMAKE_SOURCE_DIR}/src
    ${INTERCOM_INCLUDE_DIR}
)

# Link libraries
target_link_libraries(${PROJECT_NAME} PRIVATE
    Qt6::Core
    Qt6::Gui
    Qt6::Widgets
    Qt6::Multimedia
    ${INTERCOM_LIBRARY}
)

# Windows-specific settings
if(WIN32)
    set_target_properties(${PROJECT_NAME} PROPERTIES
        WIN32_EXECUTABLE TRUE
    )

    # Copy Rust DLL to output directory
    add_custom_command(TARGET ${PROJECT_NAME} POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy_if_different
        "${INTERCOM_LIB_DIR}/bin/intercom_ffi.dll"
        $<TARGET_FILE_DIR:${PROJECT_NAME}>
    )
endif()

# Install targets
install(TARGETS ${PROJECT_NAME}
    RUNTIME DESTINATION bin
)
