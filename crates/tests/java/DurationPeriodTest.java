import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.Period;

public class DurationPeriodTest {
    public static void main(String[] args) {
        // Duration
        Duration d1 = Duration.ofSeconds(3661);
        System.out.println("d1 = " + d1);
        System.out.println("d1 seconds = " + d1.getSeconds());
        System.out.println("d1 toMinutes = " + d1.toMinutes());
        System.out.println("d1 toHours = " + d1.toHours());

        Duration d2 = Duration.ofMillis(2500);
        System.out.println("d2 toMillis = " + d2.toMillis());

        Duration d3 = Duration.ofHours(2);
        System.out.println("d3 = " + d3);

        Duration d4 = Duration.ofMinutes(90);
        System.out.println("d4 = " + d4);

        // Duration.isZero / isNegative
        Duration zero = Duration.ofSeconds(0);
        System.out.println("zero.isZero = " + zero.isZero());
        System.out.println("d1.isZero = " + d1.isZero());
        System.out.println("d1.isNegative = " + d1.isNegative());

        // Duration arithmetic
        Duration d5 = d3.multipliedBy(3);
        System.out.println("d3 * 3 = " + d5);

        // Period
        Period p1 = Period.of(1, 2, 3);
        System.out.println("p1 = " + p1);
        System.out.println("p1 years = " + p1.getYears());
        System.out.println("p1 months = " + p1.getMonths());
        System.out.println("p1 days = " + p1.getDays());

        Period p2 = Period.ofDays(30);
        System.out.println("p2 = " + p2);

        Period p3 = Period.ofMonths(6);
        System.out.println("p3 = " + p3);

        Period p4 = Period.ofWeeks(2);
        System.out.println("p4 = " + p4);

        // Period.isZero
        Period pz = Period.of(0, 0, 0);
        System.out.println("pz.isZero = " + pz.isZero());

        // Period.between
        LocalDate start = LocalDate.of(2025, 1, 1);
        LocalDate end = LocalDate.of(2025, 3, 15);
        Period between = Period.between(start, end);
        System.out.println("between = " + between);
    }
}
