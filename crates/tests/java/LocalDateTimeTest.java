import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;

public class LocalDateTimeTest {
    public static void main(String[] args) {
        // Construction
        LocalDateTime dt1 = LocalDateTime.of(2025, 3, 15, 10, 30);
        System.out.println("dt1 = " + dt1);

        LocalDateTime dt2 = LocalDateTime.of(2025, 12, 25, 18, 0, 0);
        System.out.println("dt2 = " + dt2);

        // Getters
        System.out.println("year = " + dt1.getYear());
        System.out.println("month = " + dt1.getMonthValue());
        System.out.println("day = " + dt1.getDayOfMonth());
        System.out.println("hour = " + dt1.getHour());
        System.out.println("minute = " + dt1.getMinute());

        // Date/time extraction
        LocalDate d = dt1.toLocalDate();
        LocalTime t = dt1.toLocalTime();
        System.out.println("date = " + d);
        System.out.println("time = " + t);

        // Arithmetic
        LocalDateTime dt3 = dt1.plusDays(10);
        System.out.println("dt1 + 10d = " + dt3);
        LocalDateTime dt4 = dt1.plusMonths(3);
        System.out.println("dt1 + 3mo = " + dt4);
        LocalDateTime dt5 = dt1.plusHours(5);
        System.out.println("dt1 + 5h = " + dt5);

        // Comparison
        System.out.println("dt1 before dt2 = " + dt1.isBefore(dt2));

        // From LocalDate.atTime
        LocalDate date = LocalDate.of(2025, 6, 1);
        LocalDateTime dt6 = date.atTime(9, 30);
        System.out.println("date.atTime = " + dt6);

        // Parse
        LocalDateTime dt7 = LocalDateTime.parse("2025-07-04T12:00:00");
        System.out.println("parsed = " + dt7);
    }
}
