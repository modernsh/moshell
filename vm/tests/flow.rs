use crate::runner::Runner;
use vm::value::VmValue;

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
    assert_eq!(runner.eval("$res"), VmValue::Int(3))
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

    assert_eq!(runner.eval("$vec"), vec!["bar"].into())
}

#[test]
fn simple_function_call() {
    let mut runner = Runner::default();
    runner.eval("use std::{assert::*, convert::*}");
    runner.eval("fun concat(a: String, b: String) -> String = $a + $b");
    runner.eval("fun foo() -> String = concat('foo', 'bar')");

    assert_eq!(runner.eval("foo()"), "foobar".into());

    runner.eval(
        r#"\
        fun all_args(a: String, b: Int, c: Exitcode, d: Float, e: Unit, g: Bool) = {
            assert($a == "ABCDEF")
            assert($b == 7)
            assert(to_int($c) == 9)
            assert($d == 8.74)
            assert($g)
        }
    "#,
    );

    assert_eq!(
        runner.try_eval(r#"all_args("ABCDEF", 7, to_exitcode(9), 8.74, {}, true)"#),
        Ok(VmValue::Void)
    )
}

#[test]
fn operators() {
    let mut runner = Runner::default();
    runner.eval("use std::assert::*");
    runner.eval(
        r#"\
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
    "#,
    );
}

#[test]
fn str_bytes() {
    let mut runner = Runner::default();
    runner.eval("val letters = 'abcdefghijklmnopqrstuvwxy'.bytes()");
    runner.eval("$letters.push(122)");
    assert_eq!(runner.eval("$letters.get(0)"), VmValue::Int(97));
    assert_eq!(runner.eval("$letters.get(25)"), VmValue::Int(122));
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
        str_recomposed += $vec.get(0)
        while $i < $vec.len() {
            str_recomposed += " " + $vec.get($i)
            i += 1
        }
    "#,
    );

    assert_eq!(
        runner.eval("$str_recomposed"),
        "this is a string, hello strings ! ! ! babibel".into()
    )
}
