import java.time.*;

public class ZonedDateTimeTest {
    public static void main(String[] args) {
        ZoneId utc = ZoneId.of("UTC");
        ZonedDateTime zdt = ZonedDateTime.of(
            LocalDateTime.of(2024, 3, 15, 10, 30, 0),
            utc
        );
        System.out.println("year=" + zdt.getYear());
        System.out.println("month=" + zdt.getMonthValue());
        System.out.println("day=" + zdt.getDayOfMonth());
        System.out.println("hour=" + zdt.getHour());
        System.out.println("zone=" + zdt.getZone());

        ZonedDateTime next = zdt.plusDays(1);
        System.out.println("nextDay=" + next.getDayOfMonth());

        ZonedDateTime later = zdt.plusHours(3);
        System.out.println("laterHour=" + later.getHour());

        // isBefore / isAfter
        System.out.println("before=" + zdt.isBefore(next));
        System.out.println("after=" + next.isAfter(zdt));

        // ZoneId.of with offset
        ZoneId plus5 = ZoneId.of("+05:00");
        System.out.println("zoneId=" + plus5.getId());
    }
}
