use super::*;
use nom::Err;
use super::token::pinput;

//    type Result<'src, O> = std::Result< (PInput<'src>,O), Err<PError<PInput<'src>>> >;

// Adapter to simplify running test (remove indirections and replace tokens with strings)
// - create input from string
// - convert output to string
// - convert errors to ErrorKing with string parameter
fn map_res<'src, F, O>(f: F, i: &'src str) -> std::result::Result<(&'src str, O), (&'src str, PErrorKind<&'src str>)>
where
    F: Fn(PInput<'src>) -> Result<'src, O>,
{
    match f(pinput(i, "")) {
        Ok((x, y)) => Ok((x.fragment, y)),
        Err(Err::Failure(e)) => Err(map_err(e)),
        Err(Err::Error(e)) => Err(map_err(e)),
        Err(Err::Incomplete(_)) => panic!("Incomplete should never happen"),
    }
}

// Adapter to simplify error testing (convert all inputs to string)
fn map_err(err: PError<PInput>) -> (&str, PErrorKind<&str>) {
    let kind = match err.kind {
        PErrorKind::Nom(e) => PErrorKind::NomTest(format!("{:?}", e)),
        PErrorKind::NomTest(e) => PErrorKind::NomTest(e),
        PErrorKind::InvalidFormat => PErrorKind::InvalidFormat,
        PErrorKind::InvalidName(i) => PErrorKind::InvalidName(i.fragment),
        PErrorKind::UnexpectedToken(i) => PErrorKind::UnexpectedToken(i),
        PErrorKind::UnterminatedDelimiter(i) => PErrorKind::UnterminatedDelimiter(i.fragment),
        PErrorKind::UnexpectedExpressionData => PErrorKind::UnexpectedExpressionData,
    };
    (err.context.fragment, kind)
}

#[test]
fn test_spaces_and_comment() {
    assert_eq!(map_res(strip_spaces_and_comment, ""), Ok(("", ())));
    assert_eq!(map_res(strip_spaces_and_comment, "  \t\n"), Ok(("", ())));
    assert_eq!(
        map_res(strip_spaces_and_comment, "  \nhello "),
        Ok(("hello ", ()))
    );
    assert_eq!(
        map_res(
            strip_spaces_and_comment,
            "  \n#comment1 \n # comment2\n\n#comment3\n youpi"
        ),
        Ok(("youpi", ()))
    );
    assert_eq!(
        map_res(strip_spaces_and_comment, " #lastline\n#"),
        Ok(("", ()))
    );
}

#[test]
fn test_sp() {
    assert_eq!(
        map_res(sp!(pair(pidentifier, pidentifier)), "hello world"),
        Ok(("", ("hello".into(), "world".into())))
    );
    assert_eq!(
        map_res(
            sp!(pair(pidentifier, pidentifier)),
            "hello \n#pouet\n world2"
        ),
        Ok(("", ("hello".into(), "world2".into())))
    );
    assert_eq!(
        map_res(
            sp!(pair(pidentifier, pidentifier)),
            "hello  world3 #comment\n"
        ),
        Ok(("", ("hello".into(), "world3".into())))
    );
    assert_eq!(
        map_res(
            sp!(tuple((pidentifier, pidentifier))),
            "hello world"
        ),
        Ok(("", ("hello".into(), "world".into())))
    );
}

#[test]
fn test_sequence() {
    assert_eq!(
        map_res(sequence!( {
                        id1: pidentifier;
                        id2: pidentifier;
                        _x: pidentifier;
                    } => (id1,id2)
                ), "hello  world end"),
        Ok(("", ("hello".into(), "world".into())))
    );
    assert_eq!(
        map_res(sequence!( { 
                        id1: pidentifier;
                        id2: pidentifier;
                        _x: pidentifier;
                    } => (id1,id2)
               ), "hello world #end\nend"),
        Ok(("", ("hello".into(), "world".into())))
    );
    assert!(map_res(sequence!( {
                        id1: pidentifier;
                        id2: pidentifier;
                        _x: pidentifier;
                    } => (id1,id2)
              ), "hello world").is_err());
}

#[test]
fn test_pheader() {
    assert_eq!(
        map_res(pheader, "@format=21\n"),
        Ok(("", PHeader { version: 21 }))
    );
    assert_eq!(
        map_res(pheader, "#!/bin/bash\n@format=1\n"),
        Ok(("", PHeader { version: 1 }))
    );
    assert_eq!(
        map_res(pheader, "@format=21.5\n"),
        Err(("21.5\n",PErrorKind::InvalidFormat))
    );
}

#[test]
fn test_pcomment() {
    assert_eq!(
        map_res(pcomment, "##hello Herman1\n"),
        Ok((
            "",
            PComment {
                lines: vec!["hello Herman1".into()]
            }
        ))
    );
    assert_eq!(
        map_res(pcomment, "##hello Herman2\nHola"),
        Ok((
            "Hola",
            PComment {
                lines: vec!["hello Herman2".into()]
            }
        ))
    );
    assert_eq!(
        map_res(pcomment, "##hello Herman3!"),
        Ok((
            "",
            PComment {
                lines: vec!["hello Herman3!".into()]
            }
        ))
    );
    assert_eq!(
        map_res(pcomment, "##hello1\nHerman\n"),
        Ok((
            "Herman\n",
            PComment {
                lines: vec!["hello1".into()]
            }
        ))
    );
    assert_eq!(
        map_res(pcomment, "##hello2\nHerman\n## 2nd line"),
        Ok((
            "Herman\n## 2nd line",
            PComment {
                lines: vec!["hello2".into()]
            }
        ))
    );
    assert_eq!(
        map_res(pcomment, "##hello\n##Herman\n"),
        Ok((
            "",
            PComment {
                lines: vec!["hello".into(), "Herman".into()]
            }
        ))
    );
    assert!(map_res(pcomment, "hello\nHerman\n").is_err());
}

#[test]
fn test_pidentifier() {
    assert_eq!(map_res(pidentifier, "simple "), Ok((" ", "simple".into())));
    assert_eq!(map_res(pidentifier, "simple?"), Ok(("?", "simple".into())));
    assert_eq!(map_res(pidentifier, "simpl3 "), Ok((" ", "simpl3".into())));
    assert_eq!(map_res(pidentifier, "5imple "), Ok((" ", "5imple".into())));
    assert_eq!(map_res(pidentifier, "héllo "), Ok((" ", "héllo".into())));
    assert_eq!(
        map_res(pidentifier, "simple_word "),
        Ok((" ", "simple_word".into()))
    );
    assert!(map_res(pidentifier, "%imple ").is_err());
}

#[test]
fn test_penum() {
    assert_eq!(
        map_res(penum, "enum abc1 { a, b, c }"),
        Ok((
            "",
            PEnum {
                global: false,
                name: "abc1".into(),
                items: vec!["a".into(), "b".into(), "c".into()]
            }
        ))
    );
    assert_eq!(
        map_res(penum, "global enum abc2 { a, b, c }"),
        Ok((
            "",
            PEnum {
                global: true,
                name: "abc2".into(),
                items: vec!["a".into(), "b".into(), "c".into()]
            }
        ))
    );
    assert_eq!(
        map_res(penum, "enum abc3 { a, b, }"),
        Ok((
            "",
            PEnum {
                global: false,
                name: "abc3".into(),
                items: vec!["a".into(), "b".into()]
            }
        ))
    );
    assert_eq!(
        map_res(penum, "enum .abc { a, b, }"),
        Err((".abc { a, b, }", PErrorKind::InvalidName("enum")))
    );
    assert_eq!(
        map_res(penum, "enum abc a, b, }"),
        Err(("a, b, }", PErrorKind::UnexpectedToken("{")))
    );
    assert_eq!(
        map_res(penum, "enum abc { a, b, "),
        Err(("", PErrorKind::UnterminatedDelimiter("{")))
    );
}

#[test]
fn test_penum_mapping() {
    assert_eq!(
        map_res(penum_mapping,"enum abc ~> def { a -> d, b -> e, * -> f}"),
        Ok((
            "",
            PEnumMapping {
                from: "abc".into(),
                to: "def".into(),
                mapping: vec![
                    ("a".into(), "d".into()),
                    ("b".into(), "e".into()),
                    ("*".into(), "f".into()),
                ]
            }
        ))
    );
    assert_eq!(
        map_res(penum_mapping,
            "enum outcome ~> okerr { kept->ok, repaired->ok, error->error }"
        ),
        Ok((
            "",
            PEnumMapping {
                from: "outcome".into(),
                to: "okerr".into(),
                mapping: vec![
                    ("kept".into(), "ok".into()),
                    ("repaired".into(), "ok".into()),
                    ("error".into(), "error".into()),
                ]
            }
        ))
    );
}

#[test]
fn test_penum_expression() {
    assert_eq!(
        map_res(penum_expression,"a=~b:c"),
        Ok((
            "",
            PEnumExpression::Compare(Some("a".into()), Some("b".into()), "c".into())
        ))
    );
    assert_eq!(
        map_res(penum_expression, "a=~bc"),
        Ok((
            "",
            PEnumExpression::Compare(Some("a".into()), None, "bc".into())
        ))
    );
    assert_eq!(
        map_res(penum_expression, "bc"),
        Ok(("", PEnumExpression::Compare(None, None, "bc".into())))
    );
    assert_eq!(
        map_res(penum_expression, "(a =~ b:hello)"),
        Ok((
            "",
            PEnumExpression::Compare(Some("a".into()), Some("b".into()), "hello".into())
        ))
    );
    assert_eq!(
        map_res(penum_expression, "(a !~ b:hello)"),
        Ok((
            "",
            PEnumExpression::Not(Box::new(PEnumExpression::Compare(
                Some("a".into()),
                Some("b".into()),
                "hello".into()
            )))
        ))
    );
    assert_eq!(
        map_res(penum_expression, "bc&&(a||b=~hello:g)"),
        Ok((
            "",
            PEnumExpression::And(
                Box::new(PEnumExpression::Compare(None, None, "bc".into())),
                Box::new(PEnumExpression::Or(
                    Box::new(PEnumExpression::Compare(None, None, "a".into())),
                    Box::new(PEnumExpression::Compare(
                        Some("b".into()),
                        Some("hello".into()),
                        "g".into()
                    ))
                )),
            )
        ))
    );
    assert_eq!(
        map_res(penum_expression, "! a =~ hello"),
        Ok((
            "",
            PEnumExpression::Not(Box::new(PEnumExpression::Compare(
                Some("a".into()),
                None,
                "hello".into()
            )))
        ))
    );
}
