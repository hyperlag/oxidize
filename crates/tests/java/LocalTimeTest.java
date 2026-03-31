import java.time.LocalTime;

public class LocalTimeTest {
    public static void main(String[] args) {
        // Construction
        LocalTime t1 = LocalTime.of(10, 30);
        LocalTime t2 = LocalTime.of(14, 45, 30);
        System.out.println("t1 = " + t1);
        System.out.println("t2 = " + t2);

        // Getters
        System.out.println("hour = " + t2.getHour());
        System.out.println("minute = " + t2.getMinute());
        System.out.println("second = " + t2.getSecond());

        // Arithmetic
        LocalTime t3 = t1.plusHours(3);
        System.out.println("t1 + 3h = " + t3);
        LocalTime t4 = t1.plusMinutes(45);
        System.out.println("t1 + 45m = " + t4);
        LocalTime t5 = t2.minusHours(2);
        System.out.println("t2 - 2h = " + t5);

        // Comparison
        System.out.println("t1 before t2 = " + t1.isBefore(t2));
        System.out.println("t2 after t1 = " + t2.isAfter(t1));

        // Wrap around
        LocalTime t6 = LocalTime.of(23, 0);
        LocalTime t7 = t6.plusHours(3);
        System.out.println("23:00 + 3h = " + t7);

        // Parse
        LocalTime t8 = LocalTime.parse("08:15:30");
        System.out.println("parsed = " + t8);
        System.out.println("secondOfDay = " + t8.toSecondOfDay());

        // With
        LocalTime t9 = t1.withHour(20);
        System.out.println("t1 withHour(20) = " + t9);
    }
}
