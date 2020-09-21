macro_rules! ignore_result {
    ($res:expr) => {
        match $res {
            Ok(_) => (),
            Err(_) => (),
        }
    };
}
