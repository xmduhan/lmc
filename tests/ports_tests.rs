use lmc::ports::{read_port, write_port, PORT_BASE};

#[test]
fn write_port_logs_output() {
    let mut log = Vec::new();
    write_port(PORT_BASE, 42, &mut log);
    assert_eq!(log.last().unwrap(), "OUT 60000 <- 42");
}

#[test]
fn read_port_uses_input_queue() {
    let mut log = Vec::new();
    let mut queue = vec![7, 9];
    assert_eq!(read_port(PORT_BASE, 0, &mut queue, &mut log), 9);
    assert_eq!(read_port(PORT_BASE, 0, &mut queue, &mut log), 7);
    assert_eq!(read_port(PORT_BASE, 0, &mut queue, &mut log), 0);
}

#[test]
fn invalid_port_logs() {
    let mut log = Vec::new();
    let mut empty = Vec::new();
    assert_eq!(read_port(1, 0, &mut empty, &mut log), 0);
    assert!(log[0].contains("非法端口读"));
}
