#include <gtest/gtest.h>

#include <limits>
#include <string>
#include <string_view>

#include "protoyaff/default_values.yaff.h"

namespace {
constexpr std::string_view DEFAULT_TEXT = "quoted \"value\"\\path\n";
const std::string DEFAULT_BYTES{"\0\xff", 2};
}  // namespace

TEST(DefaultValues, NumericExtremesAndEscapedBytes) {
    const auto& view = protoyaff::test::DefaultValues::Default();
    EXPECT_EQ(view.min_i64(), std::numeric_limits<int64_t>::min());
    EXPECT_EQ(view.max_u64(), std::numeric_limits<uint64_t>::max());
    EXPECT_EQ(view.max_u32(), std::numeric_limits<uint32_t>::max());
    EXPECT_EQ(view.text().AsStringView(), DEFAULT_TEXT);
    EXPECT_EQ(view.raw().AsStringView(), DEFAULT_BYTES);
    EXPECT_FALSE(view.has_min_i64());
    EXPECT_FALSE(view.has_raw());
}

TEST(DefaultValues, ExplicitDefaultsRoundTrip) {
    for (auto layout : {yaff::MessageLayout::MESSAGE_LAYOUT_FLAT, yaff::MessageLayout::MESSAGE_LAYOUT_SPARSE}) {
        test::DefaultValues original;
        original.set_min_i64(std::numeric_limits<int64_t>::min());
        original.set_max_u64(std::numeric_limits<uint64_t>::max());
        original.set_max_u32(std::numeric_limits<uint32_t>::max());
        original.set_text(std::string{DEFAULT_TEXT});
        original.set_raw(DEFAULT_BYTES);
        yaff::Serializer serializer;
        serializer.EnforceDynamicAlternative(layout);
        const auto buffer = yaff::Serialize<protoyaff::test::DefaultValues>(serializer, original);
        const auto restored = yaff::ParseMessage<protoyaff::test::DefaultValues>(buffer.Data());
        EXPECT_TRUE(restored.has_min_i64());
        EXPECT_TRUE(restored.has_max_u64());
        EXPECT_TRUE(restored.has_max_u32());
        EXPECT_TRUE(restored.has_text());
        EXPECT_TRUE(restored.has_raw());
        EXPECT_EQ(restored.min_i64(), original.min_i64());
        EXPECT_EQ(restored.max_u64(), original.max_u64());
        EXPECT_EQ(restored.max_u32(), original.max_u32());
        EXPECT_EQ(restored.text(), original.text());
        EXPECT_EQ(restored.raw(), original.raw());
    }
}
