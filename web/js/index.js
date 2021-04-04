import("../pkg/index.js")
    .catch(console.error)
    .then(module => {
        document.getElementById("solve").onclick =
            (_) => {
                let model = document.getElementById("lcgs-model").value;
                let formula = document.getElementById("atl-formula").value;

                document.getElementById("result").innerText = module.check(model, formula);
            };
    });
