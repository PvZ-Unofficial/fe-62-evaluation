cmake_minimum_required(VERSION 3.18)
project(pvz-emulator LANGUAGES CXX)
include(${CMAKE_ROOT}/Modules/ExternalProject.cmake)

set(CMAKE_CXX_STANDARD 17)


aux_source_directory(${CMAKE_CURRENT_SOURCE_DIR}/object OBJ_SRC)
aux_source_directory(${CMAKE_CURRENT_SOURCE_DIR}/system SYS_SRC)
aux_source_directory(${CMAKE_CURRENT_SOURCE_DIR}/system/zombie ZOMB_SYS_SRC)
aux_source_directory(${CMAKE_CURRENT_SOURCE_DIR}/system/projectile PROJ_SYS_SRC)
aux_source_directory(${CMAKE_CURRENT_SOURCE_DIR}/system/plant PLANT_SYS_SRC)
aux_source_directory(${CMAKE_CURRENT_SOURCE_DIR}/learning LEARNING_SRC)

add_library(pvzemu STATIC
    ${OBJ_SRC}
    ${SYS_SRC}
    ${ZOMB_SYS_SRC}
    ${PROJ_SYS_SRC}
    ${PLANT_SYS_SRC}
    ${LEARNING_SRC}
    world.cpp)

SET(LIB_INCLUDE
   ${CMAKE_CURRENT_SOURCE_DIR}
   ${CMAKE_CURRENT_SOURCE_DIR}/lib
   ${CMAKE_CURRENT_SOURCE_DIR}/system
   ${CMAKE_CURRENT_SOURCE_DIR}/object
   ${CMAKE_CURRENT_SOURCE_DIR}/learning)

target_include_directories(pvzemu PUBLIC ${LIB_INCLUDE})

if(PVZEMU_BUILD_PYBIND)
    add_definitions(-DPVZEMU_BUILD_PYBIND)

    find_package(Python COMPONENTS Interpreter Development.Module REQUIRED)
    execute_process(
        COMMAND "${Python_EXECUTABLE}" -X utf8 -m pybind11 --cmakedir
        RESULT_VARIABLE PYBIND11_CMAKEDIR_RESULT
        OUTPUT_VARIABLE PYBIND11_CMAKEDIR
        OUTPUT_STRIP_TRAILING_WHITESPACE
        ENCODING UTF-8)
    if(NOT PYBIND11_CMAKEDIR_RESULT EQUAL 0)
        message(FATAL_ERROR
            "A modern pybind11 package is required; install it with: "
            "${Python_EXECUTABLE} -m pip install pybind11>=3.0")
    endif()
    string(REGEX REPLACE "^\"(.*)\"$" "\\1"
        PYBIND11_CMAKEDIR "${PYBIND11_CMAKEDIR}")
    set(pybind11_DIR "${PYBIND11_CMAKEDIR}")
    find_package(pybind11 3.0 CONFIG REQUIRED)

    pybind11_add_module(pvzemu-py MODULE
        ${OBJ_SRC}
        ${SYS_SRC}
        ${ZOMB_SYS_SRC}
        ${PROJ_SYS_SRC}
        ${PLANT_SYS_SRC}
        ${LEARNING_SRC}
        world.cpp
        pybind.cpp)

    target_include_directories(pvzemu-py PUBLIC ${LIB_INCLUDE})
    set_target_properties(pvzemu-py PROPERTIES OUTPUT_NAME pvzemu)
endif(PVZEMU_BUILD_PYBIND)

if(PVZEMU_BUILD_DEBUGGER)
    add_definitions(-DPVZEMU_BUILD_DEBUGGER)

    ExternalProject_Add(debugger-ui
        SOURCE_DIR ${CMAKE_CURRENT_SOURCE_DIR}/debugger
        CONFIGURE_COMMAND npm i
        BUILD_ALWAYS true
        BUILD_IN_SOURCE true
        BUILD_COMMAND npm run build
        INSTALL_COMMAND "")

    add_executable(debug-server ${CMAKE_CURRENT_SOURCE_DIR}/main.cpp)
    add_dependencies(debug-server debugger-ui)
    target_link_libraries(debug-server pvzemu)
endif(PVZEMU_BUILD_DEBUGGER)

option(PVZEMU_BUILD_TESTS "Build PvZ emulator regression tests" OFF)

if(PVZEMU_BUILD_TESTS)
    enable_testing()

    add_executable(petest ${CMAKE_CURRENT_SOURCE_DIR}/tests/petest.cpp)
    target_link_libraries(petest pvzemu)
    target_include_directories(petest PRIVATE ${LIB_INCLUDE})
    add_executable(peparitytest ${CMAKE_CURRENT_SOURCE_DIR}/tests/peparitytest.cpp)
    target_link_libraries(peparitytest pvzemu)
    target_include_directories(peparitytest PRIVATE ${LIB_INCLUDE})
    if(MSVC)
        set_target_properties(petest PROPERTIES LINK_FLAGS "/STACK:16777216")
        set_target_properties(peparitytest PROPERTIES LINK_FLAGS "/STACK:16777216")
    endif()

    add_test(NAME petest COMMAND petest)
    add_test(NAME peparitytest COMMAND peparitytest)
endif(PVZEMU_BUILD_TESTS)
