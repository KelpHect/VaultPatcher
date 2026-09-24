fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set("ProductName", "Vault Patcher");
        res.set("FileDescription", "Vault Patcher - Borderlands patcher and mod installer");
        let _ = res.compile();
    }
}
