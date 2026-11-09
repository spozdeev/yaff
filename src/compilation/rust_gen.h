#pragma once

#include "compiler.h"

namespace yaff::compilation {

// Produces one Rust module tree for all schemas in the IR. The generated file
// is included at crate root; imported types share this same module tree.
class RustGenerator final : public AbstractGenerator {
public:
    explicit RustGenerator(std::ostream& output, bool trusted = false);
    void Generate(const ir::IR& ir) override;

private:
    std::ostream& Output_;
    bool Trusted_;
};

}  // namespace yaff::compilation
