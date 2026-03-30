import java.util.ArrayList;
import java.util.List;

class GenericWildcard {
    static int countItems(List<?> items) {
        return items.size();
    }

    public static void main(String[] args) {
        System.out.println(countItems(new ArrayList<>()));
        List<?> list = new ArrayList<>();
        System.out.println(list.size());
    }
}
