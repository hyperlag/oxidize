import java.util.Random;
import java.util.concurrent.ThreadLocalRandom;

public class RandomTest {
    public static void main(String[] args) {
        // Seeded determinism: two Randoms with same seed produce identical sequences
        Random r1 = new Random(42);
        Random r2 = new Random(42);
        System.out.println(r1.nextInt(100) == r2.nextInt(100));  // true
        System.out.println(r1.nextInt(100) == r2.nextInt(100));  // true
        System.out.println(r1.nextBoolean() == r2.nextBoolean()); // true
        System.out.println(r1.nextDouble() == r2.nextDouble());   // true

        // Range correctness
        Random r = new Random(99);
        int v = r.nextInt(10);
        System.out.println(v >= 0 && v < 10);    // true
        double d = r.nextDouble();
        System.out.println(d >= 0.0 && d < 1.0); // true

        // Math.random() in [0.0, 1.0)
        double m = Math.random();
        System.out.println(m >= 0.0 && m < 1.0); // true

        // ThreadLocalRandom range
        int t = ThreadLocalRandom.current().nextInt(10);
        System.out.println(t >= 0 && t < 10);    // true
    }
}
