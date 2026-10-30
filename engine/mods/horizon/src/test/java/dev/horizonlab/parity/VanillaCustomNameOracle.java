package dev.horizonlab.parity;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;

/** Visit ItemStack.getStyledHoverName, including the parent rarity/italic style. */
public final class VanillaCustomNameOracle {
    public static void main(String[] args) throws Exception {
        ParityData.init();
        var output = new ArrayList<String>();
        for (String line : Files.readAllLines(Path.of(args[0]))) {
            if (!line.isBlank()) output.add(VanillaLoreTooltipOracle.spans(ParityData.stack(line).getStyledHoverName()));
        }
        Files.write(Path.of(args[1]), output);
    }
}
