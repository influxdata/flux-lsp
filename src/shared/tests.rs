use super::*;
use crate::protocol::properties::Position;

// This gives us a colorful diff.
#[cfg(test)]
use pretty_assertions::assert_eq;

#[test]
fn get_ident_name() {
    struct Case<'a> {
        src: &'a str,
        line: u32,
        character: u32,
        result: String,
    }
    let cases: Vec<Case> = vec![
        Case {
            src: r#"fn1(measurment: "cpu", bucket1: "buck2")"#,
            line: 0,
            character: 33, //buck2
            result: "bucket1".to_owned(),
        },
        Case {
            src: r#"fn1(measurment: "cpu", bucket: "buck2")"#,
            line: 0,
            character: 15, //: "cpu"
            result: "measurment".to_owned(),
        },
        Case {
            src: r#"member1."#,
            line: 0,
            character: 8, //.
            result: "member1".to_owned(),
        },
        Case {
            src: r#"from(bucket2:)"#,
            line: 0,
            character: 13, //:
            result: "bucket2".to_owned(),
        },
        Case {
            src: r#"from(bucket: ")"#,
            line: 0,
            character: 14, //"
            result: "bucket".to_owned(),
        },
        Case {
            src: r#"keys(bucket: , measurement: )"#,
            line: 0,
            character: 28, // space of measurement
            result: "measurement".to_owned(),
        },
        Case {
            src: r#"keys(bucket: , measurement: )"#,
            line: 0,
            character: 13, // space of bucket
            result: "bucket".to_owned(),
        },
        Case {
            src: r#"|> filter(fn: (r) => r._measurement == )"#,
            line: 0,
            character: 38, // ==
            result: "_measurement".to_owned(),
        },
    ];
    for c in cases {
        let result = find_ident_from_closest(
            c.src,
            &Position {
                line: c.line,
                character: c.character,
            },
        );
        assert_eq!(result, c.result);
    }
}

#[test]
fn find_bucket() {
    let b = get_bucket(r#"from(bucket: "buck2")"#);
    assert_eq!(b, "buck2");
    let b2 = get_bucket(
        r#"v1.measurementTagKeys(bucket: "buck3", measurement:)"#,
    );
    assert_eq!(b2, "buck3");
    let b3 = get_bucket("nothing");
    assert_eq!(b3, "");
}
