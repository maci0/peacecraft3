mod initializer_link_regressions {
    use pcx_script::{Item, NullHost, Prelude, SourceMap, Value, Vm, parse, resolve_with};

    #[test]
    fn initializer_binds_the_linked_bodys_parameter_names() {
        let earlier =
            parse("function Linked takes integer placeholder returns integer\nendfunction\n")
                .unwrap();
        let body = parse(
            "function Linked takes integer actual returns integer\nreturn actual + 1\nendfunction\n",
        )
        .unwrap();
        let map = parse("globals\ninteger result = Linked(7)\nendglobals\n").unwrap();
        let program = resolve_with(
            &map,
            &[
                Prelude::new("synthetic/declaration.j", &earlier),
                Prelude::new("synthetic/body.j", &body),
            ],
        )
        .unwrap();
        let mut vm = Vm::new(&program, NullHost);
        let initialized = vm.init_globals();
        assert!(
            initialized.is_ok(),
            "{initialized:?}; source={:?}",
            vm.error_source()
        );
        assert_eq!(vm.global("result"), Value::Int(8));
        assert_eq!(program.function_id("Linked").unwrap().0, 0);
    }

    #[test]
    fn initializer_implicit_return_error_uses_linked_body_span() {
        let declaration = "function Linked takes nothing returns integer\nendfunction\n";
        let body_source = "// separate body source\n\n\nfunction Linked takes nothing returns integer\nlocal integer touched = 1\nendfunction\n";
        let earlier = parse(declaration).unwrap();
        let body = parse(body_source).unwrap();
        let expected_span = match &body.items[0] {
            Item::Function(f) => f.span,
            _ => panic!("function"),
        };
        let map = parse("globals\ninteger result = Linked()\nendglobals\n").unwrap();
        let program = resolve_with(
            &map,
            &[
                Prelude::new("synthetic/declaration.j", &earlier),
                Prelude::new("synthetic/body.j", &body),
            ],
        )
        .unwrap();
        let mut vm = Vm::new(&program, NullHost);
        let error = vm.init_globals().unwrap_err();
        assert!(error.message.contains("without a value"));
        assert_eq!(vm.error_source(), Some("synthetic/body.j"));
        assert_eq!(error.span, expected_span);
        assert_eq!(error.locate(&SourceMap::new(body_source)), (4, 1));
        assert_eq!(vm.init_globals().unwrap_err(), error);
    }
}
