import java.util.EnumMap;

public class EnumMapBasic {
    enum Day { MON, TUE, WED }

    public static void main(String[] args) {
        EnumMap<Day, String> map = new EnumMap<>(Day.class);
        map.put(Day.MON, "Monday");
        map.put(Day.TUE, "Tuesday");
        map.put(Day.WED, "Wednesday");
        System.out.println(map.size());
        System.out.println(map.get(Day.MON));
        System.out.println(map.containsKey(Day.TUE));
        System.out.println(map.isEmpty());
        map.remove(Day.WED);
        System.out.println(map.size());
        System.out.println(map.containsKey(Day.WED));
    }
}
