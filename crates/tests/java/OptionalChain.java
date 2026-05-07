import java.util.List;
import java.util.Optional;
import java.util.stream.Collectors;

class OptionalChain {
    public static void main(String[] args) {
        Optional<String> opt = Optional.of("hello");
        Optional<String> empty = Optional.empty();

        // map + flatMap chain
        String upper = opt.map(s -> s.toUpperCase()).orElse("");
        System.out.println(upper);  // HELLO

        Optional<String> flatMapped = opt.flatMap(s -> Optional.of(s + " world"));
        System.out.println(flatMapped.get());  // hello world

        // flatMap on empty → empty
        Optional<String> flatEmpty = empty.flatMap(s -> Optional.of(s + " world"));
        System.out.println(flatEmpty.isPresent());  // false

        // orElseGet — lazy default
        String lazyDefault = empty.orElseGet(() -> "lazy");
        System.out.println(lazyDefault);  // lazy

        // orElseGet on present returns value, not default
        String lazyPresent = opt.orElseGet(() -> "lazy");
        System.out.println(lazyPresent);  // hello

        // orElseThrow on present — returns value without throwing
        String throwPresent = opt.orElseThrow();
        System.out.println(throwPresent);  // hello

        // orElseThrow on empty — throws NoSuchElementException
        try {
            empty.orElseThrow();
            System.out.println("no throw");
        } catch (Exception e) {
            System.out.println("caught empty");  // caught empty
        }

        // ifPresentOrElse — present branch
        opt.ifPresentOrElse(
            s -> System.out.println("present: " + s),
            () -> System.out.println("absent")
        );  // present: hello

        // ifPresentOrElse — absent branch
        empty.ifPresentOrElse(
            s -> System.out.println("present: " + s),
            () -> System.out.println("absent")
        );  // absent

        // or — falls back when empty
        Optional<String> orResult = empty.or(() -> Optional.of("fallback"));
        System.out.println(orResult.get());  // fallback

        // or — does not fall back when present
        Optional<String> orPresent = opt.or(() -> Optional.of("fallback"));
        System.out.println(orPresent.get());  // hello

        // stream on present — 1-element stream
        List<String> streamList = opt.stream().collect(Collectors.toList());
        System.out.println(streamList.size());  // 1
        System.out.println(streamList.get(0));  // hello

        // stream on empty — 0-element stream
        List<String> emptyList = empty.stream().collect(Collectors.toList());
        System.out.println(emptyList.size());  // 0
    }
}
