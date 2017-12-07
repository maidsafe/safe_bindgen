use super::*;
use std::path::Path;

macro_rules! compile {
    ($lang:expr, $rust:tt) => {
        try_compile!($lang, $rust).unwrap()
    };
}

macro_rules! try_compile {
        ($lang:expr, $rust:tt) => {{
            let rust_src = stringify!($rust);
            let rust_src = rust_src[1..rust_src.len() - 1].to_string();
            try_compile($lang, rust_src)
        }};
    }

// This is like `assert_eq`, but produces more readable output for multiline
// strings.
macro_rules! assert_multiline_eq {
        ($left:expr, $right:expr) => {{
            use $crate::colored::*;

            let left = $left;
            let right = $right;

            if left != right {
                panic!("assertion failed: `({} == {})`\n```\n{}```\n",
                       "left".red(),
                       "right".green(),
                       format_diff(&left, &right));
            }
        }}
    }

#[test]
fn non_repr_c_types_are_ignored() {
    let outputs = compile!(None, {
        pub struct Foo {
            bar: i32,
        }

        pub enum Meta {
            Foo,
            Bar,
            Baz,
        }
    });

    let actual = fetch(&outputs, "Types.cs");
    assert!(actual.is_empty());
}

#[test]
fn structs() {
    let outputs = compile!(None, {
        #[repr(C)]
        pub struct Record {
            id: u64,
            enabled: bool,
            name: *const c_char,
            random_numbers: [i32; 10],
            widget: Widget,
            gadgets: [Gadget; 100],
        }
    });

    let actual = fetch(&outputs, "Types.cs");
    let expected = indoc!(
        "using System;
         using System.Runtime.InteropServices;

         namespace Backend {
             public struct Record {
                 public ulong Id;
                 [MarshalAs(UnmanagedType.U1)]
                 public bool Enabled;
                 [MarshalAs(UnmanagedType.LPStr)]
                 public String Name;
                 [MarshalAs(UnmanagedType.ByValArray, SizeConst = 10)]
                 public int[] RandomNumbers;
                 public Widget Widget;
                 [MarshalAs(UnmanagedType.ByValArray, SizeConst = 100)]
                 public Gadget[] Gadgets;
             }

         }
         "
    );

    assert_multiline_eq!(actual, expected);
}

#[test]
fn structs_with_dynamic_array_field() {
    // It should append "Native" to the struct name, to allow writing custom
    // wrappers.

    let outputs = compile!(None, {
        #[repr(C)]
        pub struct Entry {
            key_ptr: *const u8,
            key_len: usize,
            records_ptr: *const Record,
            records_len: usize,
        }

        #[no_mangle]
        pub extern "C" fn fun(entry: Entry) {}
    });

    let actual = fetch(&outputs, "Types.cs");
    let expected = indoc!(
        "using System;
         using System.Runtime.InteropServices;

         namespace Backend {
             public struct EntryNative {
                 public IntPtr KeyPtr;
                 public ulong KeyLen;
                 public IntPtr RecordsPtr;
                 public ulong RecordsLen;
             }

         }
        "
    );
    assert_multiline_eq!(actual, expected);

    let actual = fetch(&outputs, "Backend.cs");
    let expected = indoc!(
        "using System;
         using System.Runtime.InteropServices;
         using System.Threading.Tasks;

         namespace Backend {
             public partial class Backend : IBackend {
                 #if __IOS__
                 internal const String DLL_NAME = \"__Internal\";
                 #else
                 internal const String DLL_NAME = \"backend\";
                 #endif

                 public void Fun(EntryNative entry) {
                     FunNative(entry);
                 }

                 [DllImport(DLL_NAME, EntryPoint = \"fun\")]
                 internal static extern void FunNative(EntryNative entry);

             }
         }
        "
    );
    assert_multiline_eq!(actual, expected);
}

#[test]
fn type_aliases() {
    let outputs = compile!(None, {
        pub type Id = u64;
        // Double indirection.
        pub type UserId = Id;

        #[repr(C)]
        pub struct Message {
            id: Id,
            sender_id: UserId,
            receiver_ids: [Id; 10],
        }

        #[no_mangle]
        pub extern "C" fn fun(
            id: Id,
            user_data: *mut c_void,
            cb: extern "C" fn(*mut c_void, *const FfiResult, Id),
        ) {
        }
    });

    let actual = fetch(&outputs, "Types.cs");
    let expected = indoc!(
        "using System;
         using System.Runtime.InteropServices;

         namespace Backend {
             public struct Message {
                 public ulong Id;
                 public ulong SenderId;
                 [MarshalAs(UnmanagedType.ByValArray, SizeConst = 10)]
                 public ulong[] ReceiverIds;
             }

         }
         "
    );
    assert_multiline_eq!(actual, expected);

    let actual = fetch(&outputs, "Backend.cs");
    let expected = indoc!(
        "using System;
         using System.Runtime.InteropServices;
         using System.Threading.Tasks;

         namespace Backend {
             public partial class Backend : IBackend {
                 #if __IOS__
                 internal const String DLL_NAME = \"__Internal\";
                 #else
                 internal const String DLL_NAME = \"backend\";
                 #endif

                 public Task<ulong> Fun(ulong id) {
                     var (task, userData) = Utils.PrepareTask<ulong>();
                     FunNative(id, userData, OnFfiResultULongCb);
                     return task;
                 }

                 [DllImport(DLL_NAME, EntryPoint = \"fun\")]
                 internal static extern void FunNative(ulong id, \
                                                       IntPtr userData, \
                                                       FfiResultULongCb cb);

                 #region Callbacks
                 internal delegate void FfiResultULongCb(IntPtr arg0, \
                                                         ref FfiResult arg1, \
                                                         ulong arg2);

                 #if __IOS__
                 [MonoPInvokeCallback(typeof(FfiResultULongCb))]
                 #endif
                 private static void OnFfiResultULongCb(IntPtr arg0, \
                                                        ref FfiResult arg1, \
                                                        ulong arg2) {
                     Utils.CompleteTask(arg0, ref arg1, arg2);
                 }

                 #endregion

             }
         }
         "
    );
    assert_multiline_eq!(actual, expected);
}

#[test]
fn enums() {
    let outputs = compile!(None, {
        #[repr(C)]
        pub enum Mode {
            ReadOnly,
            WriteOnly,
            ReadAndWrite,
        }

        #[repr(C)]
        pub enum Binary {
            Zero = 0,
            One = 1,
        }
    });

    let actual = fetch(&outputs, "Types.cs");
    let expected = indoc!(
        "using System;
         using System.Runtime.InteropServices;

         namespace Backend {
             public enum Mode {
                 ReadOnly,
                 WriteOnly,
                 ReadAndWrite,
             }

             public enum Binary {
                 Zero = 0,
                 One = 1,
             }

         }
        "
    );

    assert_multiline_eq!(actual, expected);
}

#[test]
fn functions_without_extern_and_no_mangle_are_ignored() {
    let outputs = compile!(None, {
        pub extern "C" fn fun1() {}

        #[no_mangle]
        pub fn fun2() {}
    });

    let actual = fetch(&outputs, "Backend.cs");
    assert!(actual.is_empty());
}

#[test]
fn functions_taking_no_callbacks() {
    let outputs = compile!(None, {
        #[no_mangle]
        pub extern "C" fn fun0(engine: *mut Engine) {}
    });

    let actual = fetch(&outputs, "Backend.cs");
    let expected = indoc!(
        "using System;
         using System.Runtime.InteropServices;
         using System.Threading.Tasks;

         namespace Backend {
             public partial class Backend : IBackend {
                 #if __IOS__
                 internal const String DLL_NAME = \"__Internal\";
                 #else
                 internal const String DLL_NAME = \"backend\";
                 #endif

                 public void Fun0(ref Engine engine) {
                     Fun0Native(ref engine);
                 }

                 [DllImport(DLL_NAME, EntryPoint = \"fun0\")]
                 internal static extern void Fun0Native(ref Engine engine);

             }
         }
        "
    );

    assert_multiline_eq!(actual, expected);
}

#[test]
fn functions_taking_one_callback() {
    let outputs = compile!(None, {
        #[no_mangle]
        pub extern "C" fn fun1(
            num: i32,
            name: *const c_char,
            user_data: *mut c_void,
            cb: extern "C" fn(user_data: *mut c_void, result: *const FfiResult),
        ) {
        }
    });

    let actual = fetch(&outputs, "Backend.cs");
    let expected = indoc!(
        "using System;
         using System.Runtime.InteropServices;
         using System.Threading.Tasks;

         namespace Backend {
             public partial class Backend : IBackend {
                 #if __IOS__
                 internal const String DLL_NAME = \"__Internal\";
                 #else
                 internal const String DLL_NAME = \"backend\";
                 #endif

                 public Task Fun1(int num, String name) {
                     var (task, userData) = Utils.PrepareTask();
                     Fun1Native(num, name, userData, OnFfiResultCb);
                     return task;
                 }

                 [DllImport(DLL_NAME, EntryPoint = \"fun1\")]
                 internal static extern void Fun1Native(\
                    int num, \
                    [MarshalAs(UnmanagedType.LPStr)] String name, \
                    IntPtr userData, \
                    FfiResultCb cb);

                 #region Callbacks
                 internal delegate void FfiResultCb(IntPtr userData, ref FfiResult result);

                 #if __IOS__
                 [MonoPInvokeCallback(typeof(FfiResultCb))]
                 #endif
                 private static void OnFfiResultCb(IntPtr userData, ref FfiResult result) {
                     Utils.CompleteTask(userData, ref result);
                 }

                 #endregion

             }
         }
        "
    );

    assert_multiline_eq!(actual, expected);
}

#[test]
fn functions_taking_multiple_callbacks() {
    // Only the native declaration should be produced.

    let outputs = compile!(None, {
        #[no_mangle]
        pub extern "C" fn fun(
            input: i32,
            user_data: *mut c_void,
            cb0: extern "C" fn(user_data: *mut c_void),
            cb1: extern "C" fn(user_data: *mut c_void,
                               result: *const FfiResult,
                               output: i32),
        ) {
        }
    });

    let actual = fetch(&outputs, "Backend.cs");
    let expected = indoc!(
        "using System;
         using System.Runtime.InteropServices;
         using System.Threading.Tasks;

         namespace Backend {
             public partial class Backend : IBackend {
                 #if __IOS__
                 internal const String DLL_NAME = \"__Internal\";
                 #else
                 internal const String DLL_NAME = \"backend\";
                 #endif

                 [DllImport(DLL_NAME, EntryPoint = \"fun\")]
                 internal static extern void FunNative(int input, \
                                                       IntPtr userData, \
                                                       NoneCb cb0, \
                                                       FfiResultIntCb cb1);

                 #region Callbacks
                 internal delegate void FfiResultIntCb(IntPtr userData, \
                                                       ref FfiResult result, \
                                                       int output);

                 internal delegate void NoneCb(IntPtr userData);

                 #endregion

             }
         }
        "
    );

    assert_multiline_eq!(actual, expected);
}

#[test]
fn functions_taking_array() {
    let outputs = compile!(None, {
        #[no_mangle]
        pub extern "C" fn fun0(data_ptr: *const u8, data_len: usize) {}

        // Params before and/or after the array
        #[no_mangle]
        pub extern "C" fn fun1(id: u64, data_ptr: *const u8, data_len: usize) {}

        // This one does not follow the naming convention and thus is not transformed
        // to array.
        #[no_mangle]
        pub extern "C" fn fun2(result: *const FfiResult, len: usize) {}
    });

    let actual = fetch(&outputs, "Backend.cs");
    let expected = indoc!(
        "using System;
         using System.Runtime.InteropServices;
         using System.Threading.Tasks;

         namespace Backend {
             public partial class Backend : IBackend {
                 #if __IOS__
                 internal const String DLL_NAME = \"__Internal\";
                 #else
                 internal const String DLL_NAME = \"backend\";
                 #endif

                 public void Fun0(byte[] data) {
                     Fun0Native(data, (ulong) data.Length);
                 }

                 [DllImport(DLL_NAME, EntryPoint = \"fun0\")]
                 internal static extern void Fun0Native(\
                    [MarshalAs(UnmanagedType.LPArray, SizeParamIndex = 1)] byte[] data, \
                    ulong dataLen\
                 );

                 public void Fun1(ulong id, byte[] data) {
                     Fun1Native(id, data, (ulong) data.Length);
                 }

                 [DllImport(DLL_NAME, EntryPoint = \"fun1\")]
                 internal static extern void Fun1Native(\
                    ulong id, \
                    [MarshalAs(UnmanagedType.LPArray, SizeParamIndex = 2)] byte[] data, \
                    ulong dataLen\
                 );

                 public void Fun2(ref FfiResult result, ulong len) {
                     Fun2Native(ref result, len);
                 }

                 [DllImport(DLL_NAME, EntryPoint = \"fun2\")]
                 internal static extern void Fun2Native(ref FfiResult result, ulong len);

             }
         }
        "
    );

    assert_multiline_eq!(actual, expected);
}

#[test]
fn functions_taking_callback_taking_const_size_array() {
    let outputs = compile!(None, {
        // Literal.
        #[no_mangle]
        pub extern "C" fn fun2(
            user_data: *mut c_void,
            cb: extern "C" fn(user_data: *mut c_void,
                              result: *const FfiResult,
                              key: [u8; 32]),
        ) {
        }

        // Named constant.
        #[no_mangle]
        pub extern "C" fn fun3(
            user_data: *mut c_void,
            cb: extern "C" fn(user_data: *mut c_void,
                              result: *const FfiResult,
                              nonce: [u8; NONCE_LEN]),
        ) {
        }
    });

    let actual = fetch(&outputs, "Backend.cs");
    let expected = indoc!(
        "using System;
         using System.Runtime.InteropServices;
         using System.Threading.Tasks;

         namespace Backend {
             public partial class Backend : IBackend {
                 #if __IOS__
                 internal const String DLL_NAME = \"__Internal\";
                 #else
                 internal const String DLL_NAME = \"backend\";
                 #endif

                 public Task<byte[]> Fun2() {
                     var (task, userData) = Utils.PrepareTask<byte[]>();
                     Fun2Native(userData, OnFfiResultByteArray32Cb);
                     return task;
                 }

                 [DllImport(DLL_NAME, EntryPoint = \"fun2\")]
                 internal static extern void Fun2Native(IntPtr userData, \
                                                        FfiResultByteArray32Cb cb);

                 public Task<byte[]> Fun3() {
                     var (task, userData) = Utils.PrepareTask<byte[]>();
                     Fun3Native(userData, OnFfiResultByteArrayNonceLenCb);
                     return task;
                 }

                 [DllImport(DLL_NAME, EntryPoint = \"fun3\")]
                 internal static extern void Fun3Native(IntPtr userData, \
                                                        FfiResultByteArrayNonceLenCb cb);

                 #region Callbacks
                 internal delegate void FfiResultByteArray32Cb(IntPtr userData, \
                                                               ref FfiResult result, \
                                                               IntPtr keyPtr);

                 #if __IOS__
                 [MonoPInvokeCallback(typeof(FfiResultByteArray32Cb))]
                 #endif
                 private static void OnFfiResultByteArray32Cb(IntPtr userData, \
                                                              ref FfiResult result, \
                                                              IntPtr keyPtr) {
                     Utils.CompleteTask(userData, \
                                        ref result, \
                                        Utils.CopyToByteArray(keyPtr, 32));
                 }

                 internal delegate void FfiResultByteArrayNonceLenCb(IntPtr userData, \
                                                                     ref FfiResult result, \
                                                                     IntPtr noncePtr);

                 #if __IOS__
                 [MonoPInvokeCallback(typeof(FfiResultByteArrayNonceLenCb))]
                 #endif
                 private static void OnFfiResultByteArrayNonceLenCb(IntPtr userData, \
                                                                    ref FfiResult result, \
                                                                    IntPtr noncePtr) {
                     Utils.CompleteTask(userData, \
                                        ref result, \
                                        Utils.CopyToByteArray(noncePtr, Constants.NONCE_LEN));
                 }

                 #endregion

             }
         }
        "
    );

    assert_multiline_eq!(actual, expected);
}

#[test]
fn functions_taking_callback_taking_dynamic_array() {
    let outputs = compile!(None, {
        // Primitive type.
        #[no_mangle]
        pub extern "C" fn fun0(
            user_data: *mut c_void,
            cb: extern "C" fn(user_data: *mut c_void,
                              result: *const FfiResult,
                              data_ptr: *const u8,
                              data_len: usize),
        ) {
        }

        // Structures.
        #[no_mangle]
        pub extern "C" fn fun1(
            user_data: *mut c_void,
            cb: extern "C" fn(user_data: *mut c_void,
                              result: *const FfiResult,
                              records_ptr: *const Record,
                              records_len: usize),
        ) {
        }
    });

    let actual = fetch(&outputs, "Backend.cs");
    let expected = indoc!(
        "using System;
         using System.Runtime.InteropServices;
         using System.Threading.Tasks;

         namespace Backend {
             public partial class Backend : IBackend {
                 #if __IOS__
                 internal const String DLL_NAME = \"__Internal\";
                 #else
                 internal const String DLL_NAME = \"backend\";
                 #endif

                 public Task<byte[]> Fun0() {
                     var (task, userData) = Utils.PrepareTask<byte[]>();
                     Fun0Native(userData, OnFfiResultByteListCb);
                     return task;
                 }

                 [DllImport(DLL_NAME, EntryPoint = \"fun0\")]
                 internal static extern void Fun0Native(IntPtr userData, \
                                                        FfiResultByteListCb cb);

                 public Task<Record[]> Fun1() {
                     var (task, userData) = Utils.PrepareTask<Record[]>();
                     Fun1Native(userData, OnFfiResultRecordListCb);
                     return task;
                 }

                 [DllImport(DLL_NAME, EntryPoint = \"fun1\")]
                 internal static extern void Fun1Native(IntPtr userData, \
                                                        FfiResultRecordListCb cb);

                 #region Callbacks
                 internal delegate void FfiResultByteListCb(IntPtr userData, \
                                                            ref FfiResult result, \
                                                            IntPtr dataPtr, \
                                                            ulong dataLen);

                 #if __IOS__
                 [MonoPInvokeCallback(typeof(FfiResultByteListCb))]
                 #endif
                 private static void OnFfiResultByteListCb(IntPtr userData, \
                                                           ref FfiResult result, \
                                                           IntPtr dataPtr, \
                                                           ulong dataLen) {
                     Utils.CompleteTask(userData, \
                                        ref result, \
                                        Utils.CopyToByteArray(dataPtr, dataLen));
                 }

                 internal delegate void FfiResultRecordListCb(IntPtr userData, \
                                                              ref FfiResult result, \
                                                              IntPtr recordsPtr, \
                                                              ulong recordsLen);

                 #if __IOS__
                 [MonoPInvokeCallback(typeof(FfiResultRecordListCb))]
                 #endif
                 private static void OnFfiResultRecordListCb(IntPtr userData, \
                                                             ref FfiResult result, \
                                                             IntPtr recordsPtr, \
                                                             ulong recordsLen) {
                     Utils.CompleteTask(userData, \
                                        ref result, \
                                        Utils.CopyToObjectArray<Record>(\
                                            recordsPtr, \
                                            recordsLen));
                 }

                 #endregion

             }
         }
        "
    );

    assert_multiline_eq!(actual, expected);
}

#[test]
fn functions_with_opaque_params() {
    let mut lang = LangCSharp::new();
    lang.add_opaque_type("Handle");

    let outputs = compile!(lang, {
        #[no_mangle]
        pub extern "C" fn fun0(handle: *const Handle) {}
    });

    let actual = fetch(&outputs, "Types.cs");
    let expected = indoc!(
        "using System;
         using System.Runtime.InteropServices;

         namespace Backend {
             #pragma warning disable CS0169
             public struct Handle {
                 private IntPtr _value;
             }

             #pragma warning restore CS0169
         }
        "
    );
    assert_multiline_eq!(actual, expected);

    let actual = fetch(&outputs, "Backend.cs");
    let expected = indoc!(
        "using System;
         using System.Runtime.InteropServices;
         using System.Threading.Tasks;

         namespace Backend {
             public partial class Backend : IBackend {
                 #if __IOS__
                 internal const String DLL_NAME = \"__Internal\";
                 #else
                 internal const String DLL_NAME = \"backend\";
                 #endif

                 public void Fun0(Handle handle) {
                     Fun0Native(handle);
                 }

                 [DllImport(DLL_NAME, EntryPoint = \"fun0\")]
                 internal static extern void Fun0Native(Handle handle);

             }
         }
        "
    );
    assert_multiline_eq!(actual, expected);
}

#[test]
fn functions_with_return_values() {
    let outputs = compile!(None, {
        #[no_mangle]
        pub extern "C" fn fun(arg: i32) -> bool {}
    });

    let actual = fetch(&outputs, "Backend.cs");
    let expected = indoc!(
        "using System;
         using System.Runtime.InteropServices;
         using System.Threading.Tasks;

         namespace Backend {
             public partial class Backend : IBackend {
                 #if __IOS__
                 internal const String DLL_NAME = \"__Internal\";
                 #else
                 internal const String DLL_NAME = \"backend\";
                 #endif

                 public bool Fun(int arg) {
                     return FunNative(arg);
                 }

                 [DllImport(DLL_NAME, EntryPoint = \"fun\")]
                 internal static extern bool FunNative(int arg);

             }
         }
        "
    );

    assert_multiline_eq!(actual, expected);
}

#[test]
fn functions_taking_out_param() {
    let outputs = compile!(None, {
        #[no_mangle]
        pub extern "C" fn fun(o_app: *mut *mut App) {}
    });

    let actual = fetch(&outputs, "Backend.cs");
    let expected = indoc!(
        "using System;
         using System.Runtime.InteropServices;
         using System.Threading.Tasks;

         namespace Backend {
             public partial class Backend : IBackend {
                 #if __IOS__
                 internal const String DLL_NAME = \"__Internal\";
                 #else
                 internal const String DLL_NAME = \"backend\";
                 #endif

                 public void Fun(out IntPtr oApp) {
                     FunNative(out oApp);
                 }

                 [DllImport(DLL_NAME, EntryPoint = \"fun\")]
                 internal static extern void FunNative(out IntPtr oApp);

             }
         }
        "
    );
    assert_multiline_eq!(actual, expected);
}

#[test]
fn constants() {
    let mut lang = LangCSharp::new();
    lang.add_const("byte", "CUSTOM", 45);

    let outputs = compile!(lang, {
        pub const NUMBER: i32 = 123;
        pub const STRING: &'static str = "hello world";
        pub const ARRAY: [u8; 4] = [0, 1, 2, 3];

        pub const STRUCT_VALUE: Record = Record {
            id: 0,
            secret_code: "xyz",
        };

        pub const STRUCT_REF: &'static Record = &Record {
            id: 1,
            secret_code: "xyz",
        };

        pub const EMPTY_STR: *const c_char = 0 as *const c_char;
    });

    let actual = fetch(&outputs, "Constants.cs");
    let expected = indoc!(
        "using System;

         namespace Backend {
             public static class Constants {
                 public const int NUMBER = 123;
                 public const String STRING = \"hello world\";
                 public static readonly byte[] ARRAY = new byte[] { 0, 1, 2, 3 };
                 public static readonly Record STRUCT_VALUE = new Record { \
                     id = 0, secretCode = \"xyz\" };
                 public static readonly Record STRUCT_REF = new Record { \
                     id = 1, secretCode = \"xyz\" };
                 public const String EMPTY_STR = \"\";
                 public const byte CUSTOM = 45;
             }
         }
        "
    );

    assert_multiline_eq!(actual, expected);
}

#[test]
fn arrays() {
    let outputs = compile!(None, {
        pub const ARRAY_SIZE: usize = 20;

        #[no_mangle]
        pub extern "C" fn fun(a: [u8; 10], b: [u8; ARRAY_SIZE]) {}
    });

    let actual = fetch(&outputs, "Backend.cs");
    let expected = indoc!(
        "using System;
         using System.Runtime.InteropServices;
         using System.Threading.Tasks;

         namespace Backend {
             public partial class Backend : IBackend {
                 #if __IOS__
                 internal const String DLL_NAME = \"__Internal\";
                 #else
                 internal const String DLL_NAME = \"backend\";
                 #endif

                 public void Fun(byte[] a, byte[] b) {
                     FunNative(a, b);
                 }

                 [DllImport(DLL_NAME, EntryPoint = \"fun\")]
                 internal static extern void FunNative(\
                     [MarshalAs(UnmanagedType.ByValArray, SizeConst = 10)] \
                     byte[] a, \
                     [MarshalAs(UnmanagedType.ByValArray, SizeConst = (int) Constants.ARRAY_SIZE)] \
                     byte[] b);

             }
         }
        "
    );

    assert_multiline_eq!(actual, expected);
}

#[test]
fn interface() {
    let outputs = compile!(None, {
        #[no_mangle]
        pub extern "C" fn fun(
            enabled: bool,
            user_data: *mut c_void,
            cb: extern "C" fn(user_data: *mut c_void, result: *const FfiResult),
        ) {
        }
    });

    let actual = fetch(&outputs, "IBackend.cs");
    let expected = indoc!(
        "using System;
         using System.Runtime.InteropServices;
         using System.Threading.Tasks;

         namespace Backend {
             public partial interface IBackend {
                 Task Fun(bool enabled);
             }
         }
        "
    );

    assert_multiline_eq!(actual, expected);
}

fn try_compile<T: Into<Option<LangCSharp>>>(
    lang: T,
    rust_src: String,
) -> Result<HashMap<PathBuf, String>, Vec<Error>> {
    use parse;
    use syntax;

    let session = syntax::parse::ParseSess::new();
    let ast = syntax::parse::parse_crate_from_source_str("lib.rs".to_string(), rust_src, &session)
        .unwrap();

    let mut outputs = Outputs::default();
    let mut lang = lang.into().unwrap_or_else(|| LangCSharp::new());

    parse::parse_mod(&mut lang, &ast.module, &mut outputs)?;
    lang.finalise_output(&mut outputs)?;

    Ok(outputs)
}

fn fetch<'a>(outputs: &'a HashMap<PathBuf, String>, name: &str) -> &'a str {
    outputs.get(Path::new(name)).map(String::as_str).unwrap_or(
        "",
    )
}

fn format_diff(left: &str, right: &str) -> String {
    use diff;
    use std::fmt::Write;
    use colored::*;

    let mut output = String::new();

    for res in diff::lines(left, right) {
        match res {
            diff::Result::Left(line) => writeln!(output, "{}{}", "-".red(), line.red()).unwrap(),
            diff::Result::Right(line) => {
                writeln!(output, "{}{}", "+".green(), line.green()).unwrap()
            }
            diff::Result::Both(line, _) => writeln!(output, " {}", line.white()).unwrap(),
        };
    }

    output
}