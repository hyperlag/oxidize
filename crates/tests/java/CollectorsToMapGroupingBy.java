import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;

public class CollectorsToMapGroupingBy {
    public static void main(String[] args) {
        List<String> words = Arrays.asList("a", "b", "c");

        // toMap: map each word to its length
        Map<String, Integer> wordLengths = words.stream()
                .collect(Collectors.toMap(w -> w, w -> w.length()));
        System.out.println(wordLengths.size());        // 3
        System.out.println(wordLengths.get("a"));      // 1
        System.out.println(wordLengths.get("b"));      // 1

        // groupingBy: group words by length
        Map<Integer, List<String>> byLength = words.stream()
                .collect(Collectors.groupingBy(w -> w.length()));
        List<String> len1 = byLength.get(1);
        System.out.println(len1.size());               // 3
    }
}
