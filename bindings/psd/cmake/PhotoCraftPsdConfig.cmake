if(NOT TARGET PhotoCraftPsd::Psd)
  get_filename_component(_psd_prefix "${CMAKE_CURRENT_LIST_DIR}/../../.." ABSOLUTE)
  add_library(PhotoCraftPsd::Psd SHARED IMPORTED)
  set_target_properties(PhotoCraftPsd::Psd PROPERTIES
    INTERFACE_INCLUDE_DIRECTORIES "${_psd_prefix}/include")
  if(WIN32)
    set_target_properties(PhotoCraftPsd::Psd PROPERTIES
      IMPORTED_LOCATION "${_psd_prefix}/bin/photocraft_psd_bindings.dll"
      IMPORTED_IMPLIB "${_psd_prefix}/lib/photocraft_psd_bindings.dll.lib")
  else()
    set_target_properties(PhotoCraftPsd::Psd PROPERTIES
      IMPORTED_LOCATION "${_psd_prefix}/lib/${CMAKE_SHARED_LIBRARY_PREFIX}photocraft_psd_bindings${CMAKE_SHARED_LIBRARY_SUFFIX}")
  endif()
  add_library(PhotoCraftPsd::PsdStatic STATIC IMPORTED)
  if(WIN32)
    set(_psd_static "photocraft_psd_bindings.lib")
  else()
    set(_psd_static "${CMAKE_STATIC_LIBRARY_PREFIX}photocraft_psd_bindings${CMAKE_STATIC_LIBRARY_SUFFIX}")
  endif()
  separate_arguments(_psd_native_libs NATIVE_COMMAND "${_psd_native_libs}")
  set_target_properties(PhotoCraftPsd::PsdStatic PROPERTIES
    IMPORTED_LOCATION "${_psd_prefix}/lib/${_psd_static}"
    INTERFACE_INCLUDE_DIRECTORIES "${_psd_prefix}/include"
    INTERFACE_LINK_LIBRARIES "${_psd_native_libs}")
  unset(_psd_static)
  unset(_psd_prefix)
endif()
unset(_psd_native_libs)
