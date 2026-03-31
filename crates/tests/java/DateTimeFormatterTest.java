import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.time.format.DateTimeFormatter;

public class DateTimeFormatterTest {
    public static void main(String[] args) {
        DateTimeFormatter fmt1 = DateTimeFormatter.ofPattern("yyyy/MM/dd");
        LocalDate date = LocalDate.of(2025, 3, 15);
        System.out.println("formatted date = " + date.format(fmt1));

        DateTimeFormatter fmt2 = DateTimeFormatter.ofPattern("HH:mm:ss");
        LocalTime time = LocalTime.of(14, 30, 45);
        System.out.println("formatted time = " + time.format(fmt2));

        DateTimeFormatter fmt3 = DateTimeFormatter.ofPattern("yyyy-MM-dd HH:mm");
        LocalDateTime dt = LocalDateTime.of(2025, 12, 25, 18, 0);
        System.out.println("formatted dt = " + dt.format(fmt3));

        // Pattern with literal text
        DateTimeFormatter fmt4 = DateTimeFormatter.ofPattern("dd/MM/yyyy");
        System.out.println("euro format = " + date.format(fmt4));
    }
}
