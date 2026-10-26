//! Fixed C11 names and Rust 2024 strict/reserved keywords. The C source is
//! WG14 N1570 §§6.4.1, 7.1.3, 7.31 and Annex B, not the host's headers:
//! https://www.open-std.org/jtc1/sc22/wg14/www/docs/n1570.pdf
//! Rust strict/reserved words are distinct from contextual weak keywords:
//! https://doc.rust-lang.org/1.99.1/reference/keywords.html
//! In particular raw, safe and macro_rules remain valid ordinary identifiers.

pub(super) fn is_reserved(value: &str) -> bool {
    KEYWORDS.split_ascii_whitespace().any(|name| name == value)
        || C11_NAMES.split_ascii_whitespace().any(|name| name == value)
        || MATH_NAMES.split_ascii_whitespace().any(|name| {
            value == name
                || value.strip_suffix('f') == Some(name)
                || value.strip_suffix('l') == Some(name)
        })
        || future_c_names(value)
}
const KEYWORDS: &str = "
auto break case char const continue default do double else enum extern float for goto
if inline int long register restrict return short signed sizeof static struct switch
typedef union unsigned void volatile while
as async await crate dyn false fn impl in let loop match mod move mut pub ref self Self
super trait true type unsafe use where abstract become box final gen macro override
priv try typeof unsized virtual yield
";

// Families in §7.31 are below. Exact names here are the remaining C11 library
// namespace (including types/macros reserved when their standard header is used).
const C11_NAMES: &str = "
assert static_assert NDEBUG complex imaginary I CMPLX CMPLXF CMPLXL
errno fenv_t fexcept_t feclearexcept fegetexceptflag feraiseexcept fesetexceptflag
fetestexcept fegetround fesetround fegetenv feholdexcept fesetenv feupdateenv
FLT_ROUNDS FLT_EVAL_METHOD FLT_RADIX DECIMAL_DIG
FLT_MANT_DIG DBL_MANT_DIG LDBL_MANT_DIG FLT_DECIMAL_DIG DBL_DECIMAL_DIG LDBL_DECIMAL_DIG
FLT_DIG DBL_DIG LDBL_DIG FLT_MIN_EXP DBL_MIN_EXP LDBL_MIN_EXP
FLT_MIN_10_EXP DBL_MIN_10_EXP LDBL_MIN_10_EXP FLT_MAX_EXP DBL_MAX_EXP LDBL_MAX_EXP
FLT_MAX_10_EXP DBL_MAX_10_EXP LDBL_MAX_10_EXP FLT_MAX DBL_MAX LDBL_MAX
FLT_EPSILON DBL_EPSILON LDBL_EPSILON FLT_MIN DBL_MIN LDBL_MIN
FLT_TRUE_MIN DBL_TRUE_MIN LDBL_TRUE_MIN FLT_HAS_SUBNORM DBL_HAS_SUBNORM LDBL_HAS_SUBNORM
imaxdiv_t imaxabs imaxdiv wcstoimax wcstoumax
and and_eq bitand bitor compl not not_eq or or_eq xor xor_eq
CHAR_BIT SCHAR_MIN SCHAR_MAX UCHAR_MAX CHAR_MIN CHAR_MAX MB_LEN_MAX
SHRT_MIN SHRT_MAX USHRT_MAX LONG_MIN LONG_MAX ULONG_MAX LLONG_MIN LLONG_MAX ULLONG_MAX
lconv setlocale localeconv
float_t double_t HUGE_VAL HUGE_VALF HUGE_VALL INFINITY NAN
FP_INFINITE FP_NAN FP_NORMAL FP_SUBNORMAL FP_ZERO FP_FAST_FMA FP_FAST_FMAF FP_FAST_FMAL
FP_ILOGB0 FP_ILOGBNAN MATH_ERRNO MATH_ERREXCEPT math_errhandling fpclassify signbit
jmp_buf setjmp longjmp sig_atomic_t signal raise
alignas alignof va_list va_start va_arg va_copy va_end
bool true false ptrdiff_t size_t max_align_t wchar_t NULL offsetof
PTRDIFF_MIN PTRDIFF_MAX SIG_ATOMIC_MIN SIG_ATOMIC_MAX SIZE_MAX WCHAR_MIN WCHAR_MAX WINT_MIN WINT_MAX
FILE fpos_t EOF FOPEN_MAX FILENAME_MAX L_tmpnam TMP_MAX BUFSIZ SEEK_CUR SEEK_END SEEK_SET
stderr stdin stdout remove rename tmpfile tmpnam fclose fflush fopen freopen
setbuf setvbuf fprintf fscanf printf scanf snprintf sprintf sscanf vfprintf vfscanf
vprintf vscanf vsnprintf vsprintf vsscanf fgetc fgets fputc fputs getc getchar putc
putchar puts ungetc fread fwrite fgetpos fseek fsetpos ftell rewind clearerr feof ferror perror
EXIT_FAILURE EXIT_SUCCESS RAND_MAX MB_CUR_MAX div_t ldiv_t lldiv_t
atof atoi atol atoll rand srand aligned_alloc calloc free malloc realloc abort atexit
at_quick_exit exit getenv quick_exit system bsearch qsort abs labs llabs div ldiv lldiv
mblen mbtowc wctomb mbstowcs wcstombs noreturn
thread_local once_flag ONCE_FLAG_INIT TSS_DTOR_ITERATIONS call_once
clock_t time_t timespec tm CLOCKS_PER_SEC clock difftime mktime time timespec_get asctime ctime gmtime localtime
char16_t char32_t mbstate_t mbrtoc16 c16rtomb mbrtoc32 c32rtomb wint_t WEOF
fwprintf fwscanf swprintf swscanf vfwprintf vfwscanf vswprintf vswscanf vwprintf vwscanf
wprintf wscanf fgetwc fgetws fputwc fputws fwide getwc getwchar putwc putwchar ungetwc
wmemchr wmemcmp wmemcpy wmemmove wmemset btowc wctob mbsinit mbrlen mbrtowc wcrtomb mbsrtowcs
wctrans_t wctype_t wctrans wctype
";
// Every listed real/complex function has the standard float/long-double forms.
// The final complex names are the exact future reservations in §7.31.1.
const MATH_NAMES: &str = "
acos asin atan atan2 cos sin tan acosh asinh atanh cosh sinh tanh
exp exp2 expm1 frexp ilogb ldexp log log10 log1p log2 logb modf scalbn scalbln
cbrt fabs hypot pow sqrt erf erfc lgamma tgamma
ceil floor nearbyint rint lrint llrint round lround llround trunc
fmod remainder remquo copysign nan nextafter nexttoward fdim fmax fmin fma
cacos casin catan ccos csin ctan cacosh casinh catanh ccosh csinh ctanh
cexp clog cabs cpow csqrt carg cimag conj cproj creal
cerf cerfc cexp2 cexpm1 clog10 clog1p clog2 clgamma ctgamma
";
fn future_c_names(value: &str) -> bool {
    let next = |prefix: &str| {
        value
            .strip_prefix(prefix)
            .and_then(|rest| rest.bytes().next())
    };
    [
        "is", "to", "str", "mem", "wcs", "atomic_", "memory_", "cnd_", "mtx_", "thrd_", "tss_",
    ]
    .iter()
    .any(|prefix| next(prefix).is_some_and(|ch| ch.is_ascii_lowercase()))
        || ["FE_", "LC_", "SIG", "SIG_", "ATOMIC_", "TIME_"]
            .iter()
            .any(|prefix| next(prefix).is_some_and(|ch| ch.is_ascii_uppercase()))
        || next("E").is_some_and(|ch| ch.is_ascii_digit() || ch.is_ascii_uppercase())
        || ["PRI", "SCN"]
            .iter()
            .any(|prefix| next(prefix).is_some_and(|ch| ch.is_ascii_lowercase() || ch == b'X'))
        || ((value.starts_with("int") || value.starts_with("uint")) && value.ends_with("_t"))
        || ((value.starts_with("INT") || value.starts_with("UINT"))
            && ["_MAX", "_MIN", "_C"]
                .iter()
                .any(|suffix| value.ends_with(suffix)))
}
