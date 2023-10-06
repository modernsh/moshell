use crate::runner::Runner;
use pretty_assertions::assert_eq;
use vm::value::VmValue;
use vm::VmError;

mod runner;

#[test]
fn break_loop() {
    let mut runner = Runner::default();
    runner.eval(
        "\
        var res = 1
        while true {
            res += 1
            break;
            res += 10
        }
        res += 1
    ",
    );
    assert_eq!(runner.eval("$res"), Some(VmValue::Int(3)))
}

#[test]
fn test_assertion() {
    let mut runner = Runner::default();
    assert_eq!(
        runner.try_eval(r#"std::assert::assert(true)"#),
        Ok(Some(VmValue::Void))
    );
    assert_eq!(
        runner.try_eval(r#"std::assert::assert(false)"#),
        Err(VmError::Panic)
    );
}
#[test]
fn closure() {
    let mut runner = Runner::default();
    runner.eval(
        r#"\
        use std::assert::*
        val vec = {
            var captured = 'bar'
            val vec = "".split(' ')
            assert($vec.len() == 1)
            fun foo(arg: String) = {
                assert($captured == 'bar')
                assert($vec.len() == 1)
                $vec.pop()
                $vec.push($captured)
                captured = $arg
                assert($captured == 'baz')
            }
            foo('baz')
            assert($captured == 'baz')
            $vec
        }
    "#,
    );

    assert_eq!(runner.eval("$vec"), Some(vec!["bar"].into()))
}

#[test]
fn simple_function_call() {
    let mut runner = Runner::default();
    runner.eval("use std::{assert::*, convert::*, memory::*}");
    runner.eval("fun concat(a: String, b: String) -> String = $a + $b");
    runner.eval("fun foo() -> String = concat('foo', 'bar')");

    assert_eq!(runner.eval("foo()"), Some("foobar".into()));

    runner.eval(
        r#"\
        fun all_args(a: String, b: Int, c: Exitcode, d: Float, e: Unit, g: Bool) = {
            assert($a == "ABCDEF")
            assert($b == 7)
            assert($c.to_int() == 9)
            assert($d == 8.74)
            assert($g)
        }
    "#,
    );

    assert_eq!(
        runner.try_eval(
            r#"
            all_args("ABCDEF", 7, 9.to_exitcode(), 8.74, {}, true)
            assert(empty_operands())
        "#
        ),
        Ok(Some(VmValue::Void))
    )
}

#[test]
fn operators() {
    let mut runner = Runner::default();
    runner.eval("use std::assert::*");
    runner.eval(
        "
        assert(1 + 1 == 2)
        assert(1 - 1 == 0)
        assert(1 > 1 == false)
        assert(1 > 0 == true)

        assert(1 >= 1 == true)
        assert(1 >= 2 == false)
        assert(2 >= 1 == true)

        assert(1 <= 0 == false)
        assert(1 <= 1 == true)
        assert(1 <= 2 == true)

        assert(1 <= 2 == true)
        assert(1 <= 2 == true)
        assert(1 <= 2 == true)

        assert(1 / 3 == 0)
        assert(5.0 / 2.0 == 2.5)
        assert(5 % 2 == 1)
        assert(10 * (0 - 8) == 0 - 80)
    ",
    );
}

#[test]
fn str_bytes() {
    let mut runner = Runner::default();
    runner.eval("val letters = 'abcdefghijklmnopqrstuvwxy'.bytes()");
    runner.eval("$letters.push(122)");
    assert_eq!(runner.eval("$letters[0]"), Some(VmValue::Int(97)));
    assert_eq!(runner.eval("$letters[25]"), Some(VmValue::Int(122)));
}

#[test]
fn str_split() {
    let mut runner = Runner::default();
    runner.eval("val str = 'this is a string, hello strings ! ! !'");
    runner.eval("val vec = $str.split(' ')");
    runner.eval("$vec.push('babibel')");
    runner.eval(
        r#"
        var str_recomposed = ""
        var i = 1
        str_recomposed += $vec[0]
        while $i < $vec.len() {
            str_recomposed += " " + $vec[$i]
            i += 1
        }
    "#,
    );

    assert_eq!(
        runner.eval("$str_recomposed"),
        Some("this is a string, hello strings ! ! ! babibel".into())
    )
}
