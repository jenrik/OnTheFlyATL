import("../pkg/index.js")
    .catch(console.error)
    .then(module => {
        document.atl_checker = module;
    });
