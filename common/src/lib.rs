use xdg::BaseDirectories;


pub fn socket() -> std::path::PathBuf {
    let xdg_dirs = BaseDirectories::new().unwrap();
    let socket_path = xdg_dirs.place_runtime_file("unikey").unwrap();
    socket_path
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     #[test]
//     fn it_works() {
//         let result = add(2, 2);
//         assert_eq!(result, 4);
//     }
// }
