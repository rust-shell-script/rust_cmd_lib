These declarative macros are deprecated for two major reasons:
- source_text! macro is using include_str!, which will include all the
  original source file into the final binary
- Using CARGO_MANIFEST_DIR environment variable is fragile, see issue #12
