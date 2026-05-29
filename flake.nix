{
  description = "Nami (波) — GPU-rendered TUI browser";

  inputs.substrate.url = "github:pleme-io/substrate";

  outputs = { substrate, ... }: substrate.rust.tool { src = ./.; };
}
